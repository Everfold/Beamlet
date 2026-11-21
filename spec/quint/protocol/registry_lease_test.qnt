// L3 Worker 租约与单调状态格场景测试.
module registry_lease_test {
  import registry_lease.* from "./registry_lease"

  // 断线重连推进 epoch 后, 旧租约立即失效且旧 epoch 心跳被拒绝, 直到新 epoch 收到新心跳才恢复.
  run reconnectRequiresNewEpochHeartbeatTest =
    init
      .then(observePresence(1, 1, 1, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == Selected({ worker: 1, source: 1 }))
      .then(disconnectLink(1))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
      .then(reconnectLink(1))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
      .then(observePresence(1, 1, 1, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
      .then(observePresence(1, 2, 1, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == Selected({ worker: 1, source: 1 }))

  // 单调状态格: 进入 Draining 或 Offline 后, 乱序到达的 Ready 永远无法复活该会话.
  run monotonicModeBlocksStaleReadyTest =
    init
      .then(observePresence(1, 1, 1, ModeReady))
      .then(observePresence(1, 1, 1, ModeDraining))
      .then(observePresence(2, 1, 1, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
      .then(observePresence(1, 1, 1, ModeOffline))
      .then(observePresence(1, 1, 1, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)

  // 多来源同一 Worker, 来源 1 断线后来源 2 仍保持可路由, 直至本地 TTL 过期.
  run multiSourceIsolationAndTtlExpiryTest =
    init
      .then(observePresence(1, 1, 1, ModeReady))
      .then(observePresence(2, 1, 1, ModeReady))
      .then(disconnectLink(1))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == Selected({ worker: 1, source: 2 }))
      .then(advanceClock)
      .then(advanceClock)
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
}
