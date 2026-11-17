use super::*;
use uuid::Uuid;

/// 验证终端文本只发给选中角色，EOF 不广播或提交额外文本。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn terminal_sends_to_one_selected_role_and_eof_does_not_broadcast() {
    let (send, input) = mpsc::channel(8);
    let (a, mut a_input) = mpsc::channel(2);
    let (b, mut b_input) = mpsc::channel(2);
    let first = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let second = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let targets = vec![
        Target {
            instance: first,
            label: None,
            send: a,
        },
        Target {
            instance: second,
            label: None,
            send: b,
        },
    ];
    let host = beamlet_host::Host::new(Default::default()).unwrap();
    let shutdown = host.shutdown();
    for line in [
        "unselected".into(),
        "/authority 2".into(),
        "  second  ".into(),
        format!("/authority {}", first.role_id),
        "first".into(),
    ] {
        send.send(Ok(line)).await.unwrap();
    }
    drop(send);
    route(targets, input, shutdown).await.unwrap();
    assert_eq!(a_input.recv().await.unwrap().unwrap(), "first");
    assert_eq!(b_input.recv().await.unwrap().unwrap(), "  second  ");
    assert!(a_input.recv().await.is_none());
    assert!(b_input.recv().await.is_none());
}
