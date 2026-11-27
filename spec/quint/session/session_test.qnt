// RoleSession、Presence 单调状态与本地租约场景测试.
module session_test {
  import session.* from "./session"

  // 持续收到 Accepting 快照续期本地租约, 未刷新则 TTL 到期退出可路由.
  run heartbeatRenewsLeaseAndExpiresWithoutRefreshTest =
    init
      .then(observePresence(ModeAccepting))
      .expect(isSessionRoutable(entry, now))
      .then(advanceClock)
      .then(observePresence(ModeAccepting))
      .then(advanceClock)
      .expect(isSessionRoutable(entry, now))
      .then(advanceClock)
      .expect(not(isSessionRoutable(entry, now)))

  // Draining 与 Offline 立即停止可路由, 且拒绝状态回退.
  run drainingAndOfflineStopRoutingAndRejectRegressionTest =
    init
      .then(observePresence(ModeAccepting))
      .then(observePresence(ModeDraining))
      .expect(not(isSessionRoutable(entry, now)))
      .then(observePresence(ModeAccepting))
      .expect(and {
        rejectedRegression,
        not(isSessionRoutable(entry, now)),
        entry == Entry({ mode: ModeDraining, lastSeenAt: 0 }),
      })
      .then(observePresence(ModeOffline))
      .then(observePresence(ModeDraining))
      .expect(and {
        rejectedRegression,
        not(isSessionRoutable(entry, now)),
        entry == Entry({ mode: ModeOffline, lastSeenAt: 0 }),
      })

  // 直接收到 Offline 快照亦不可路由, 也不允许再回退为 Accepting.
  run directOfflineSessionNeverRoutableTest =
    init
      .then(observePresence(ModeOffline))
      .expect(not(isSessionRoutable(entry, now)))
      .then(observePresence(ModeAccepting))
      .expect(and {
        rejectedRegression,
        not(isSessionRoutable(entry, now)),
        entry == Entry({ mode: ModeOffline, lastSeenAt: 0 }),
      })
}
