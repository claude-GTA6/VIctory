//! Structured buffers through the raw views the host makes of them: record
//! counts read as data and indexed loads and atomics. Synthetic GFX10
//! programs lowered to DXIL, validated by `dxil.dll` when present, and run
//! on the host GPU and WARP. The cases come from the reviews of upstream
//! Marvisak/VIctory#1762 (AlanQiang0418, rminzao).

use super::host::{Dispatch, Host, Root, Stride, View};
use super::{HostShader, lower, validator};
use crate::native::{
    ComputeIo, HwStage, NativeProgram, ResourceType, RootParam, RootType, StageIo, UsedResource,
};
use crate::{io, ir, resource, structure};

/// User data: s0 a table with a structured SRV (12-byte elements), s2 a
/// table with a structured UAV (8-byte elements). Each lane loads dword 1
/// of SRV element `lane` and adds it atomically to dword 1 of UAV element
/// `lane`: indexed accesses the translation reads through raw handles at
/// `index * stride + offset`, as the host makes structured views raw.
const STRUCTURED_LOAD_ATOMIC_WAVE32: &[u32] = &[
    0xBE84_0300, // s_mov_b32 s4, s0
    0xBE85_0380, // s_mov_b32 s5, 0
    0xF408_0202,
    0xFA00_0000, // s_load_dwordx4 s[8:11], s[4:5], 0x0
    0xBE8C_0302, // s_mov_b32 s12, s2
    0xBE8D_0380, // s_mov_b32 s13, 0
    0xF408_0406,
    0xFA00_0000, // s_load_dwordx4 s[16:19], s[12:13], 0x0
    0xBF8C_C07F, // s_waitcnt lgkmcnt(0)
    0xE030_2004,
    0x8002_0200, // buffer_load_dword v2, v0, s[8:11], 0 idxen offset:4
    0xBF8C_3F70, // s_waitcnt vmcnt(0)
    0xE0C8_2004,
    0x8004_0200, // buffer_atomic_add v2, v0, s[16:19], 0 idxen offset:4
    0xBF81_0000, // s_endpgm
];

fn param(root: RootType, user_data: u8) -> RootParam {
    RootParam {
        root,
        table_offset_dwords: 0,
        sgprs: 1,
        base_register: 0,
        user_data,
        slot: user_data,
    }
}

/// An entry at offset 0 of the table in user data `user_data`.
fn entry(kind: ResourceType, dwords: u8, user_data: u8) -> UsedResource {
    UsedResource {
        kind,
        root: RootType::Table,
        table_offset_dwords: 0,
        lower_bound: 0,
        stride: 0,
        dwords,
        user_data,
        slot: user_data,
    }
}

fn build(
    words: &[u32],
    layout: Vec<RootParam>,
    used: Vec<UsedResource>,
    threads: u32,
) -> HostShader {
    let native = NativeProgram {
        stage: HwStage::Cs,
        wave64: false,
        vgprs: 8,
        layout,
        used,
        io: StageIo::Cs(ComputeIo {
            rsrc2: 3 << 1,
            threads: [threads, 1, 1],
        }),
        code: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
    };
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let st = structure::structure(&p).expect("structure");
    let res = resource::track(&p, &native.layout, native.stage, Some(&st)).expect("track");
    let io = io::raise(&p, &native).expect("io::raise");
    let lowered = lower::lower(&lower::Parts {
        native: &native,
        p: &p,
        st: &st,
        io: &io,
        res: &res,
        inputs: Vec::new(),
        outputs: Vec::new(),
        prims: Vec::new(),
        mesh_stride: 0,
    })
    .expect("lower");
    let shader = HostShader {
        stage: HwStage::Cs,
        dxil: lowered.dxil,
        user_data: lowered.user_data,
        samplers: lowered.samplers,
        static_samplers: Vec::new(),
        root_cbvs: lowered.root_cbvs,
        root_srvs: lowered.root_srvs,
        root_uavs: lowered.root_uavs,
        root_indexed_views: res.root_indexed_views,
    };
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        v.validate(&mut shader.dxil.clone())
            .expect("DXIL validation failed");
    } else {
        eprintln!("no dxil.dll; validation skipped");
    }
    shader
}

/// A buffer descriptor's dword 2 (its record count) read as data, beside a
/// load or store through the same descriptor: records of the structure
/// stride for structured views however the code addresses them (by index,
/// at byte offsets, by scalar loads), bytes for raw views, texels for typed
/// ones; and the right element through the raw views the host makes. The
/// cases are AlanQiang0418's regression from the review of upstream #1762: on an
/// RTX 5080 byte-offset accesses counted 192 bytes instead of 16 records,
/// and indexed loads through structured handles of raw views read element 0
/// or 1 for strides 8 to 24.
#[test]
fn a_buffer_descriptors_record_count_reads_as_its_views_kind() {
    use Stride::{Raw, Structured, Typed};
    // 48 dwords, floats 100.0, 101.0, ...: 16 records of 12 bytes, 12
    // RGBA32F texels.
    let data: Vec<u32> = (0..48).map(|i| (100.0f32 + i as f32).to_bits()).collect();
    // v3 = 1 (index) or 12 (byte); the access leaves its value in v2.
    let load_index = [0x7E06_0281, 0xE030_2000, 0x8002_0203]; // buffer_load_dword v2, v3, s[8:11], 0 idxen
    let load_byte = [0x7E06_028C, 0xE030_1000, 0x8002_0203]; // buffer_load_dword v2, v3, s[8:11], 0 offen
    let load_scalar = [0xF420_0504, 0xFA00_000C, 0xBF8C_C07F, 0x7E04_0214]; // s_buffer_load_dword s20, s[8:11], 12; v2 = s20
    let load_typed = [0x7E06_0281, 0xE000_2000, 0x8002_0203]; // buffer_load_format_x v2, v3, s[8:11], 0 idxen
    // v2 = 42 stored to the first dword of record 1.
    let store_index = [0x7E04_02AA, 0x7E06_0281, 0xE070_2000, 0x8002_0203];
    let store_byte = [0x7E04_02AA, 0x7E06_028C, 0xE070_1000, 0x8002_0203];
    // (what, the access, the view, unordered, [record count, value]).
    type Case<'a> = (&'a str, &'a [u32], Stride, bool, [u32; 2]);
    let cases: [Case; 11] = [
        ("4-byte records by index", &load_index, Structured(4), false, [48, data[1]]),
        ("8-byte records by index", &load_index, Structured(8), false, [24, data[2]]),
        ("12-byte records by index", &load_index, Structured(12), false, [16, data[3]]),
        ("16-byte records by index", &load_index, Structured(16), false, [12, data[4]]),
        ("24-byte records by index", &load_index, Structured(24), false, [8, data[6]]),
        ("12-byte records at a byte offset", &load_byte, Structured(12), false, [16, data[3]]),
        ("12-byte records by a scalar load", &load_scalar, Structured(12), false, [16, data[3]]),
        ("raw bytes", &load_byte, Raw, false, [192, data[3]]),
        ("typed texels", &load_typed, Typed, false, [12, data[4]]),
        ("12-byte unordered records by index", &store_index, Structured(12), true, [16, 42]),
        ("12-byte unordered records at a byte offset", &store_byte, Structured(12), true, [16, 42]),
    ];
    let _one = super::valu_tests::ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let hosts: Vec<_> = [("hardware", false), ("warp", true)]
        .into_iter()
        .filter_map(|(label, warp)| Host::new(warp).ok().map(|h| (label, h)))
        .collect();
    assert!(!hosts.is_empty(), "neither hardware nor WARP SM 6.6 device available");
    let mut wrong = Vec::new();
    for (name, access, view, unordered, [count, value]) in cases {
        let mut words = vec![
            0xBE84_0300, // s_mov_b32 s4, s0
            0xBE85_0380, // s_mov_b32 s5, 0
            0xF408_0202,
            0xFA00_0000, // s_load_dwordx4 s[8:11], s[4:5], 0x0
            0xF408_0402,
            0xFA00_0020, // s_load_dwordx4 s[16:19], s[4:5], 0x20
            0xBF8C_C07F, // s_waitcnt lgkmcnt(0)
            0x7E02_020A, // v_mov_b32 v1, s10 (dword 2: the record count)
        ];
        words.extend_from_slice(access);
        words.extend([
            0xBF8C_3F70, // s_waitcnt vmcnt(0)
            0xE070_1000,
            0x8004_0100, // buffer_store_dword v1, v0, s[16:19], 0 offen
            0xE070_1004,
            0x8004_0200, // buffer_store_dword v2, v0, s[16:19], 0 offen offset:4
            0xBF81_0000, // s_endpgm
        ]);
        let kind = if unordered { ResourceType::Uav } else { ResourceType::Srv };
        let stride = match view {
            Structured(bytes) => bytes,
            Typed => 16,
            _ => 0,
        };
        let output = UsedResource {
            table_offset_dwords: 8,
            ..entry(ResourceType::Uav, 4, 0)
        };
        let shader = build(
            &words,
            vec![param(RootType::Table, 0)],
            vec![UsedResource { stride, ..entry(kind, 4, 0) }, output],
            1,
        );
        let input = if unordered {
            View::RawUav(48)
        } else {
            View::Buffer { data: &data, stride: view }
        };
        let views = [input, View::RawUav(2)];
        for (label, host) in &hosts {
            let (root, pso) = host
                .compute_pipeline(&shader)
                .unwrap_or_else(|e| panic!("{label} {name}: compute_pipeline failed: {e}"));
            let ud = vec![0; shader.user_data_dwords().max(1) as usize];
            let out = host
                .dispatch(
                    &root,
                    &pso,
                    &Dispatch {
                        roots: vec![(0, Root::Constants(ud))],
                        views: &views,
                        samplers: &[],
                        groups: [1, 1, 1],
                    },
                )
                .unwrap_or_else(|e| panic!("{label} {name}: dispatch failed: {e}"));
            let got = out.last().cloned().unwrap_or_default();
            if got != [count, value] {
                wrong.push(format!("{label} {name}: {got:?}, want [{count}, {value}]"));
            }
            if unordered && out[0].get(3) != Some(&value) {
                wrong.push(format!("{label} {name}: byte 12 holds {:?}, want {value}", out[0].get(3)));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// From rminzao's review of upstream #1762: indexed loads and atomics of structured
/// buffers (strides from their used-resource entries) read and write the
/// right bytes through the raw views the host makes of structured views,
/// and declare raw handles (a structured handle on a raw view is undefined
/// in D3D12).
#[test]
fn indexed_loads_and_atomics_of_structured_buffers_read_raw_views() {
    let structured = |kind, user_data, stride| UsedResource {
        stride,
        ..entry(kind, 4, user_data)
    };
    let shader = build(
        STRUCTURED_LOAD_ATOMIC_WAVE32,
        vec![param(RootType::Table, 0), param(RootType::Table, 2)],
        vec![
            structured(ResourceType::Srv, 0, 12),
            structured(ResourceType::Uav, 2, 8),
        ],
        32,
    );
    // 32 elements of 12 bytes: (100 + e, 200 + e, 300 + e).
    let data: Vec<u32> = (0..32).flat_map(|e| [100 + e, 200 + e, 300 + e]).collect();
    let views = [
        View::RawUav(64),
        View::Buffer {
            data: &data,
            stride: Stride::Structured(12),
        },
    ];
    let _one = super::valu_tests::ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut ran = 0;
    for (label, warp) in [("hardware", false), ("warp", true)] {
        let Ok(host) = Host::new(warp) else {
            eprintln!("no SM 6.6 {label} device; skipped");
            continue;
        };
        let (root, pso) = host
            .compute_pipeline(&shader)
            .unwrap_or_else(|e| panic!("{label}: compute_pipeline failed: {e}"));
        let mut ud = vec![0u32; shader.user_data_dwords().max(1) as usize];
        for u in &shader.user_data {
            // The SRV's table at heap index 1 (byte 32), the UAV's at 0.
            ud[u.index as usize] = if u.slot == 0 { 32 } else { 0 };
        }
        let words = host
            .dispatch(
                &root,
                &pso,
                &Dispatch {
                    roots: vec![(0, Root::Constants(ud))],
                    views: &views,
                    samplers: &[],
                    groups: [1, 1, 1],
                },
            )
            .unwrap_or_else(|e| panic!("{label}: dispatch failed: {e}"));
        let want: Vec<u32> = (0..32).flat_map(|l| [0, 200 + l]).collect();
        assert_eq!(words[0], want, "{label}");
        ran += 1;
    }
    assert!(ran > 0, "neither hardware nor WARP SM 6.6 device available");
}
