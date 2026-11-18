use beamlet_api::{ReplyError, Responder};
use std::{
    error::Error,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::oneshot;

/// 回复入口测试的拥有所有权的数据, 故意不实现 Clone.
#[derive(Debug, PartialEq, Eq)]
struct Answer(
    /// 需要通过一次性回复移动的字节内容.
    Vec<u8>,
);

/// 验证一次性回复可移动非 Clone 数据, 并等待适配器发送完成.
#[tokio::test]
async fn custom_adapter_moves_owned_data_and_waits_for_send_completion() {
    let (sent, received) = oneshot::channel();
    let (release, ready) = oneshot::channel();
    let reply = Responder::new(move |answer: Answer| async move {
        sent.send(answer)
            .map_err(|_| ReplyError::new(io::Error::other("receiver closed")))?;
        ready.await.map_err(ReplyError::new)
    });
    let sending = tokio::spawn(reply.send(Answer(vec![1, 2, 3])));
    assert_eq!(received.await.unwrap(), Answer(vec![1, 2, 3]));
    assert!(!sending.is_finished(), "send must await adapter completion");
    release.send(()).unwrap();
    sending.await.unwrap().unwrap();
}

/// 验证回复发送失败保留原错误来源且不自动重试.
#[tokio::test]
async fn failures_preserve_the_original_source_without_retrying() {
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let reply = Responder::new(move |_: ()| async move {
        called.fetch_add(1, Ordering::SeqCst);
        Err(ReplyError::new(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "disconnected",
        )))
    });
    let error = reply.send(()).await.unwrap_err();
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// 验证丢弃未使用的回复入口只释放资源, 不触发发送.
#[test]
fn dropping_an_unused_reply_releases_resources_without_sending() {
    let (sender, mut receiver) = oneshot::channel::<()>();
    let reply = Responder::new(move |()| async move {
        sender
            .send(())
            .map_err(|_| ReplyError::new(io::Error::other("receiver closed")))
    });
    drop(reply);
    assert_eq!(
        receiver.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    );
}
