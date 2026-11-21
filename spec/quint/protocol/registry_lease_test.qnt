// L3 Worker 登记表租约与连接世代场景测试.
module registry_lease_test {
  import registry_lease.* from "./registry_lease"

  // 断线再重连推进 epoch 后, 旧租约立即失效且旧 epoch 心跳被拒绝, 直到新 epoch 收到新心跳才恢复.
  run reconnectRequiresNewEpochHeartbeatTest =
    init
      .then(observePresence(1, 1, 1, 1, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == Selected({ worker: 1, source: 1 }))
      .then(disconnectLink(1))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
      .then(reconnectLink(1))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
      .then(observePresence(1, 1, 1, 2, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
      .then(observePresence(1, 2, 1, 2, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == Selected({ worker: 1, source: 1 }))

  // Offline 期间收到相同或更小 seq 的旧 Ready 心跳不能复活, 只有更大 seq 的 Ready 才能恢复.
  run offlineBlocksStaleReadyReplayTest =
    init
      .then(observePresence(1, 1, 1, 2, ModeOffline))
      .then(observePresence(1, 1, 1, 1, ModeReady))
      .then(observePresence(2, 1, 1, 2, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
      .then(observePresence(1, 1, 1, 3, ModeReady))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == Selected({ worker: 1, source: 1 }))

  // 多来源同一 Worker, 来源 1 断线后来源 2 仍保持可路由, 且重复相同 seq 不延长租约.
  run multiSourceIsolationAndNoExtendOnDuplicateSeqTest =
    init
      .then(observePresence(1, 1, 1, 1, ModeReady))
      .then(advanceClock)
      .then(observePresence(2, 1, 1, 1, ModeReady))
      .then(disconnectLink(1))
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == Selected({ worker: 1, source: 2 }))
      .then(advanceClock)
      .then(selectAction(ExactWorker(1)))
      .expect(lastSelection == NoneSelected)
}
