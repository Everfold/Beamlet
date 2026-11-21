// L3 Worker 租约与单调状态格模型.
// 对应 04-protocol.md: 单会话状态单调演进 (Ready -> Draining -> Offline), 本地单调时钟 TTL, 连接 epoch 隔离与多入口隔离.
module registry_lease {
  type WorkerMode =
    | ModeReady
    | ModeDraining
    | ModeOffline

  type LinkState = {
    epoch: int,
    connected: bool,
  }

  type SourceLease =
    | NoLease
    | ActiveLease({ epoch: int, receivedAt: int })

  type RegistryEntry =
    | AbsentEntry
    | Entry({
        mode: WorkerMode,
        leases: int -> SourceLease,
      })

  type TargetSelection =
    | AnyWorker
    | ExactWorker(int)

  type SelectionOutcome =
    | NoneSelected
    | Selected({ worker: int, source: int })

  pure val WORKERS = Set(1)
  pure val SOURCES = Set(1, 2)
  pure val MAX_EPOCHS = 2
  pure val MAX_NOW = 4
  pure val LEASE_TIMEOUT = 2

  // 单调状态格序: Ready(1) < Draining(2) < Offline(3).
  pure def modeRank(mode: WorkerMode): int =
    match mode {
      | ModeReady => 1
      | ModeDraining => 2
      | ModeOffline => 3
    }

  pure def joinMode(current: WorkerMode, incoming: WorkerMode): WorkerMode =
    if (modeRank(incoming) > modeRank(current)) incoming else current

  // 判断某来源上的单条租约在当前连接状态和本地时间下是否新鲜有效.
  pure def leaseIsFresh(
    link: LinkState,
    lease: SourceLease,
    currentTime: int
  ): bool =
    match lease {
      | NoLease => false
      | ActiveLease(l) => and {
          link.connected,
          l.epoch == link.epoch,
          currentTime - l.receivedAt < LEASE_TIMEOUT,
        }
    }

  // 判断 (worker, source) 在当前登记视图下是否允许接收新任务.
  pure def isRoutable(
    linksMap: int -> LinkState,
    entriesMap: int -> RegistryEntry,
    currentTime: int,
    w: int,
    src: int
  ): bool =
    match entriesMap.get(w) {
      | AbsentEntry => false
      | Entry(e) =>
          if (e.mode != ModeReady) false
          else leaseIsFresh(linksMap.get(src), e.leases.get(src), currentTime)
    }

  // 按稳定来源编号顺序返回 Worker 当前第一个有效交付出口 (0 表示无有效出口).
  pure def firstFreshSource(
    linksMap: int -> LinkState,
    entry: RegistryEntry,
    currentTime: int
  ): int =
    match entry {
      | AbsentEntry => 0
      | Entry(e) =>
          if (e.mode != ModeReady) 0
          else if (leaseIsFresh(linksMap.get(1), e.leases.get(1), currentTime)) 1
          else if (leaseIsFresh(linksMap.get(2), e.leases.get(2), currentTime)) 2
          else 0
    }

  // 评估候选选择: Exact 不回退到其他 Worker, Any 按固定顺序挑选首个可路由候选.
  pure def evalSelect(
    linksMap: int -> LinkState,
    entriesMap: int -> RegistryEntry,
    currentTime: int,
    target: TargetSelection
  ): SelectionOutcome =
    match target {
      | ExactWorker(w) => {
          val src = firstFreshSource(linksMap, entriesMap.get(w), currentTime)
          if (src == 0) NoneSelected else Selected({ worker: w, source: src })
        }
      | AnyWorker => {
          val s1 = firstFreshSource(linksMap, entriesMap.get(1), currentTime)
          if (s1 != 0) Selected({ worker: 1, source: s1 })
          else NoneSelected
        }
    }

  // 应用一条在连接在线且世代匹配时收到的 Presence 公告 (单调状态合流).
  pure def applyObserve(
    entry: RegistryEntry,
    observedEpoch: int,
    mode: WorkerMode,
    src: int,
    currentTime: int
  ): RegistryEntry =
    match entry {
      | AbsentEntry =>
          Entry({
            mode: mode,
            leases: SOURCES.mapBy(s =>
              if (s == src and mode == ModeReady)
                ActiveLease({ epoch: observedEpoch, receivedAt: currentTime })
              else NoLease
            ),
          })
      | Entry(e) => {
          val nextMode = joinMode(e.mode, mode)
          val nextLeases =
            if (nextMode == ModeReady)
              e.leases.put(src, ActiveLease({ epoch: observedEpoch, receivedAt: currentTime }))
            else e.leases
          Entry({
            mode: nextMode,
            leases: nextLeases,
          })
        }
    }

  // --- 状态机变量 ---

  var links: int -> LinkState
  var entries: int -> RegistryEntry
  var now: int
  var lastSelection: SelectionOutcome
  var everMaxRank: int -> int

  action init: bool = all {
    links' = SOURCES.mapBy(_ => { epoch: 1, connected: true }),
    entries' = WORKERS.mapBy(_ => AbsentEntry),
    now' = 0,
    lastSelection' = NoneSelected,
    everMaxRank' = WORKERS.mapBy(_ => 0),
  }

  // 观察 Worker 公告 (允许旧 epoch 或乱序 Ready 到达以验证隔离与单调性).
  action observePresence(
    src: int,
    observedEpoch: int,
    w: int,
    mode: WorkerMode
  ): bool = {
    val link = links.get(src)
    val accepted = link.connected and link.epoch == observedEpoch
    val oldEntry = entries.get(w)
    val nextEntry =
      if (accepted) applyObserve(oldEntry, observedEpoch, mode, src, now)
      else oldEntry
    val nextRank = match nextEntry {
      | AbsentEntry => everMaxRank.get(w)
      | Entry(e) =>
          if (modeRank(e.mode) > everMaxRank.get(w)) modeRank(e.mode)
          else everMaxRank.get(w)
    }
    all {
      entries' = entries.put(w, nextEntry),
      links' = links,
      now' = now,
      lastSelection' = NoneSelected,
      everMaxRank' = everMaxRank.put(w, nextRank),
    }
  }

  action disconnectLink(src: int): bool = {
    val link = links.get(src)
    all {
      link.connected,
      links' = links.put(src, { ...link, connected: false }),
      entries' = entries,
      now' = now,
      lastSelection' = NoneSelected,
      everMaxRank' = everMaxRank,
    }
  }

  action reconnectLink(src: int): bool = {
    val link = links.get(src)
    all {
      not(link.connected),
      link.epoch < MAX_EPOCHS,
      links' = links.put(src, { epoch: link.epoch + 1, connected: true }),
      entries' = entries,
      now' = now,
      lastSelection' = NoneSelected,
      everMaxRank' = everMaxRank,
    }
  }

  action advanceClock: bool = all {
    now < MAX_NOW,
    now' = now + 1,
    entries' = entries,
    links' = links,
    lastSelection' = NoneSelected,
    everMaxRank' = everMaxRank,
  }

  action selectAction(target: TargetSelection): bool = all {
    lastSelection' = evalSelect(links, entries, now, target),
    links' = links,
    entries' = entries,
    now' = now,
    everMaxRank' = everMaxRank,
  }

  action step: bool = {
    nondet src = SOURCES.oneOf()
    nondet w = WORKERS.oneOf()
    nondet ep = 1.to(MAX_EPOCHS).oneOf()
    nondet mode = Set(ModeReady, ModeDraining, ModeOffline).oneOf()
    nondet target = Set(AnyWorker, ExactWorker(1)).oneOf()
    any {
      observePresence(src, ep, w, mode),
      disconnectLink(src),
      reconnectLink(src),
      advanceClock,
      selectAction(target),
    }
  }

  // --- 安全不变量 (对应 04-protocol.md 中的 INV-REG-*) ---

  // INV-REG-01: 任意可路由或被选中的 (worker, source) 必须满足链路在线, 世代匹配, 未超时且状态为 Ready.
  val inv_reg_01_epoch_and_connection_isolation: bool = and {
    match lastSelection {
      | NoneSelected => true
      | Selected(s) => isRoutable(links, entries, now, s.worker, s.source)
    },
    WORKERS.forall(w => SOURCES.forall(src =>
      isRoutable(links, entries, now, w, src) implies
        match entries.get(w) {
          | AbsentEntry => false
          | Entry(e) =>
              match e.leases.get(src) {
                | NoLease => false
                | ActiveLease(l) => and {
                    links.get(src).connected,
                    l.epoch == links.get(src).epoch,
                    now - l.receivedAt < LEASE_TIMEOUT,
                    e.mode == ModeReady,
                  }
              }
        }
    )),
  }

  // INV-REG-02: 单会话状态严格单调不可逆, 一旦进入 Draining 或 Offline 永不被乱序 Ready 复活.
  val inv_reg_02_monotonic_mode_never_revives: bool =
    WORKERS.forall(w =>
      match entries.get(w) {
        | AbsentEntry => true
        | Entry(e) => and {
            modeRank(e.mode) >= everMaxRank.get(w),
            (everMaxRank.get(w) >= 2) implies
              SOURCES.forall(src => not(isRoutable(links, entries, now, w, src))),
          }
      }
    )

  // INV-REG-03: 多传输来源租约相互隔离, 任一来源断线不影响其他保持在线且有效世代上的来源.
  val inv_reg_03_multi_source_fault_isolation: bool =
    WORKERS.forall(w => SOURCES.forall(src =>
      match entries.get(w) {
        | AbsentEntry => true
        | Entry(e) =>
            match e.leases.get(src) {
              | NoLease => true
              | ActiveLease(l) =>
                  (e.mode == ModeReady
                    and links.get(src).connected
                    and l.epoch == links.get(src).epoch
                    and now - l.receivedAt < LEASE_TIMEOUT) implies
                    isRoutable(links, entries, now, w, src)
            }
      }
    ))

  val invariants: bool = and {
    inv_reg_01_epoch_and_connection_isolation,
    inv_reg_02_monotonic_mode_never_revives,
    inv_reg_03_multi_source_fault_isolation,
  }

  // --- 可达性见证 ---

  val witnessSelected: bool =
    lastSelection != NoneSelected

  val witnessReconnectInvalidatesOldEpoch: bool =
    SOURCES.exists(s =>
      links.get(s).epoch == 2 and
        match entries.get(1) {
          | AbsentEntry => false
          | Entry(e) =>
              match e.leases.get(s) {
                | NoLease => false
                | ActiveLease(l) => l.epoch == 1
              }
        }
    )

  val witnessOfflineBlocksOldReady: bool =
    match entries.get(1) {
      | AbsentEntry => false
      | Entry(e) => e.mode == ModeOffline
    }

  val witnessMultiSourceFailover: bool =
    not(links.get(1).connected) and isRoutable(links, entries, now, 1, 2)
}
