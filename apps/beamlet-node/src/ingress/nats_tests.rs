//! 真实 broker 验证线上入口, 与内存 Authority 测试互补.
use super::{Ingress as _, live::Inputs};
use crate::test_support::Broker;
use std::{future::Future, time::Duration};
use tokio::{sync::mpsc, task::JoinHandle};

async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .expect("NATS 入口测试未在十秒内完成")
}

fn request(client: &async_nats::Client, bytes: Vec<u8>) -> JoinHandle<async_nats::Message> {
    let client = client.clone();
    tokio::spawn(async move {
        client
            .request("beamlet.reverse", bytes.into())
            .await
            .unwrap()
    })
}

#[tokio::test]
#[ignore = "需要 nats-server: cargo test -p beamlet-node -- --ignored"]
async fn nats_validation_local_eof_and_reply_after_stop() {
    within(async {
        let (_broker, client) = Broker::start().await;
        let (local, receiver) = mpsc::channel(2);
        let mut input = Inputs::with_input(client.clone(), receiver).await.unwrap();
        for text in ["", " Hello 世界 "] {
            local.send(Ok(text.into())).await.unwrap();
            let received = input.receive().await.unwrap().unwrap();
            assert_eq!(received.text, text);
            received.reply.send("local", "结果".into()).await.unwrap();
        }
        drop(local);
        {
            let next = input.receive();
            tokio::pin!(next);
            assert!(futures::poll!(&mut next).is_pending(), "本地 EOF 不能结束外部入口");
        }
        // 不带回复地址的消息被忽略, 非 UTF-8 请求只收到输入错误, 不进入 Authority.
        client.publish("beamlet.reverse", "fire and forget".into()).await.unwrap();
        client.flush().await.unwrap();
        let invalid = request(&client, vec![0xff]);
        tokio::select! {
            unexpected = input.receive() => panic!("无效输入不能交给 Authority: {}", unexpected.is_ok()),
            response = invalid => assert_eq!(response.unwrap().payload.as_ref(), "错误: 消息必须是 UTF-8 文本".as_bytes()),
        }
        // 取消上一轮 receive 后继续接收, 最后的回复入口在 stop 后仍有效.
        let response = request(&client, " after EOF 世界 ".as_bytes().to_vec());
        let received = input.receive().await.unwrap().unwrap();
        assert_eq!(received.text, " after EOF 世界 ");
        input.stop().await.unwrap();
        input.stop().await.unwrap();
        assert!(input.receive().await.unwrap().is_none());
        received.reply.send("operation", "界世 FOE retfa ".into()).await.unwrap();
        input.flush().await.unwrap();
        assert_eq!(response.await.unwrap().payload.as_ref(), "界世 FOE retfa ".as_bytes());
        let error = client.request("beamlet.reverse", "new".into()).await.unwrap_err();
        assert_eq!(error.kind(), async_nats::RequestErrorKind::NoResponders);
    }).await;
}

#[tokio::test]
#[ignore = "需要 nats-server: cargo test -p beamlet-node -- --ignored"]
async fn nats_queue_group_selection_and_out_of_order_replies() {
    within(async {
        let (_broker, client) = Broker::start().await;
        let (_, local_a) = mpsc::channel(1);
        let (_, local_b) = mpsc::channel(1);
        let mut a = Inputs::with_input(client.clone(), local_a).await.unwrap();
        let mut b = Inputs::with_input(client.clone(), local_b).await.unwrap();
        let first_response = request(&client, b"first".to_vec());
        let second_response = request(&client, b"second".to_vec());
        let mut received = Vec::new();
        for _ in 0..2 {
            received.push(tokio::select! {
                request = a.receive() => request.unwrap().unwrap(),
                request = b.receive() => request.unwrap().unwrap(),
            });
        }
        assert_ne!(
            received[0].text, received[1].text,
            "队列组不能向两个入口重复交付同一请求"
        );
        // 不依赖 broker 选择谁, 按业务请求识别并逆序回复.
        received.sort_by(|left, right| right.text.cmp(&left.text));
        for request in received {
            request
                .reply
                .send("operation", format!("reply:{}", request.text))
                .await
                .unwrap();
        }
        assert_eq!(
            second_response.await.unwrap().payload.as_ref(),
            b"reply:second"
        );
        assert_eq!(
            first_response.await.unwrap().payload.as_ref(),
            b"reply:first"
        );
        // 完成的回复证明 broker 已处理这两次发布, 两入口均不应再有副本.
        for input in [&mut a, &mut b] {
            let next = input.receive();
            tokio::pin!(next);
            assert!(futures::poll!(&mut next).is_pending());
        }
        a.stop().await.unwrap();
        b.stop().await.unwrap();
        a.flush().await.unwrap();
    })
    .await;
}

#[tokio::test]
#[ignore = "需要 nats-server: cargo test -p beamlet-node -- --ignored"]
async fn nats_local_read_errors_are_not_eof() {
    within(async {
        let (_broker, client) = Broker::start().await;
        let (local, receiver) = mpsc::channel(1);
        let mut input = Inputs::with_input(client, receiver).await.unwrap();
        local
            .send(Err(std::io::Error::other("test read failure")))
            .await
            .unwrap();
        let error = match input.receive().await {
            Err(error) => error,
            Ok(_) => panic!("读取错误不能当作 EOF 或有效请求"),
        };
        assert_eq!(error.to_string(), "读取本地输入失败");
        input.stop().await.unwrap();
        input.flush().await.unwrap();
    })
    .await;
}
