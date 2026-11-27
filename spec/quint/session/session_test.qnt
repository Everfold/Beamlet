// RoleSession 固定能力集、Presence 单调状态与本地租约场景测试.
module session_test {
  import session.* from "./session"

  // 持续收到同能力集 Accepting 快照续期本地租约, 未刷新则 TTL 到期退出可路由.
  run heartbeatRenewsLeaseAndExpiresWithoutRefreshTest =
    init
      .then(observePresence(1, ModeAccepting))
      .expect(isSessionRoutable(entry, now))
      .then(advanceClock)
      .then(observePresence(1, ModeAccepting))
      .then(advanceClock)
      .expect(isSessionRoutable(entry, now))
      .then(advanceClock)
      .expect(not(isSessionRoutable(entry, now)))

  // 能力集合篡改与状态回退均被拒绝, Draining 与 Offline 立即停止可路由.
  run rejectCapMutationAndModeRegressionTest =
    init
      .then(observePresence(1, ModeAccepting))
      .then(observePresence(2, ModeAccepting))
      .expect(and {
        rejectedInvalid,
        isSessionRoutable(entry, now),
        entry == Entry({ caps: 1, mode: ModeAccepting, lastSeenAt: 0 }),
      })
      .then(observePresence(1, ModeDraining))
      .expect(not(isSessionRoutable(entry, now)))
      .then(observePresence(1, ModeAccepting))
      .expect(and {
        not(isSessionRoutable(entry, now)),
        entry == Entry({ caps: 1, mode: ModeDraining, lastSeenAt: 0 }),
      })
      .then(observePresence(1, ModeOffline))
      .then(observePresence(1, ModeDraining))
      .expect(entry == Entry({ caps: 1, mode: ModeOffline, lastSeenAt: 0 }))

  // 直接收到 Offline 快照亦不可路由, 也不允许再回退为 Accepting.
  run directOfflineSessionNeverRoutableTest =
    init
      .then(observePresence(1, ModeOffline))
      .expect(not(isSessionRoutable(entry, now)))
      .then(observePresence(1, ModeAccepting))
      .expect(and {
        rejectedInvalid,
        not(isSessionRoutable(entry, now)),
        entry == Entry({ caps: 1, mode: ModeOffline, lastSeenAt: 0 }),
      })
}
