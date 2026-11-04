//! E2E：在 host 上用一个 mock USB 传输跑完整“读取 UIM lock”会话。
//!
//! 这个测试证明：操作状态机（握手 -> 切服务 -> 0x54 -> 解析 0x74 -> 0x56 退出）
//! 与 USB 后端（rusb / MCU USBH）可以彻底分离，且分片到达的端点数据也正确缓冲。

use auroku_core_nostd::mock::MockTransport;
use auroku_core_nostd::{
    read_uimlk, AuRokkuError, FrameIo, CMD_EXIT_UIMLK, CMD_READ_STATE,
};

/// 真实设备 readuimlk 的帧序列（简化：略去 drain / 0xD8 / Type2Auth challenge）：
///   TX 1C 05 -> RX 1C 05
///   TX E7    -> RX E7
///   TX 54    -> RX 74 <control> <8 nibble bytes>
///   TX 56    -> RX 56
fn read_uimlk_state_response(control: u8, value: [u8; 4]) -> [u8; 10] {
    let mut frame = [0u8; 10];
    frame[0] = 0x74;
    frame[1] = control;
    for (i, byte) in value.iter().enumerate() {
        frame[2 + i * 2] = byte >> 4;
        frame[3 + i * 2] = byte & 0x0F;
    }
    frame
}

#[test]
fn read_uimlk_end_to_end_with_fragmented_frames() {
    let state = read_uimlk_state_response(0x00, [0x12, 0x34, 0x56, 0x78]);
    let transport = MockTransport::new(&[
        &[0x1C, 0x05], // base handshake echo
        &[0xE7],       // UIMLK switch echo
        &state,        // 0x74 state frame
        &[0x56],       // exit acknowledgement
    ])
    .with_chunk(3); // 端点每次最多回 3 字节：0x74 帧会被拆成 4 次 read

    let mut io = FrameIo::new(transport);
    let outcome = read_uimlk(&mut io).expect("uimlk read should succeed");

    assert_eq!(outcome.state.control, 0);
    assert_eq!(outcome.state.lock_value_hex, "12345678");
    assert!(!outcome.state.binding_cleared);
    assert!(outcome.exit_acknowledged);

    let transport = io.into_inner();
    assert_eq!(
        transport.writes,
        vec![
            vec![0x1C, 0x05],
            vec![0xE7],
            vec![CMD_READ_STATE],
            vec![CMD_EXIT_UIMLK],
        ],
        "protocol command order must match main read_uimlk"
    );
    assert_eq!(transport.remaining(), 0, "all scripted bytes consumed");
}

#[test]
fn read_uimlk_reports_handshake_mismatch() {
    let transport = MockTransport::new(&[&[0x1C, 0x04]]);
    let mut io = FrameIo::new(transport);
    let err = read_uimlk(&mut io).unwrap_err();
    match err {
        AuRokkuError::Runtime(msg) => assert!(msg.contains("handshake echo mismatch")),
        other => panic!("expected Runtime error, got {other:?}"),
    }
    // 握手失败后不应该继续发 0xE7/0x54/0x56。
    let transport = io.into_inner();
    assert_eq!(transport.writes, vec![vec![0x1C, 0x05]]);
}

#[test]
fn read_uimlk_times_out_when_device_goes_silent() {
    // 只给 base 回显，之后设备不再响应。
    let transport = MockTransport::new(&[&[0x1C, 0x05]]);
    let mut io = FrameIo::new(transport);
    let err = read_uimlk(&mut io).unwrap_err();
    assert!(matches!(err, AuRokkuError::Timeout(_)));
    let transport = io.into_inner();
    assert_eq!(
        transport.writes,
        vec![vec![0x1C, 0x05], vec![0xE7]]
    );
}
