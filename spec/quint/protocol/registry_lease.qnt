// L3 Worker 登记表租约与连接世代协议模型.
// 对应 05-roles.md 与 Authority Registry 逻辑: 单调 sequence, 本地时钟 TTL, 4x 保留期, 连接 epoch 隔离与多入口隔离.
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
    | ActiveLease({ epoch: int, receivedAt: int, leaseSeq: int })

  type RegistryEntry =
    | AbsentEntry
    | Entry({
        mode: WorkerMode,
        maxSeq: int,
        lastAcceptedAt: int,
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
  pure val SEQS = Set(1, 2, 3)
  pure val MAX_EPOCHS = 2
  pure val MAX_NOW = 4
  pure val LEASE_TIMEOUT = 2
  pure val RETENTION = 4

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

  // 判断给定来源上的 presence 公告在当前连接世代与序号规则下是否可被接受.
  pure def canObserve(
    link: LinkState,
    entry: RegistryEntry,
    observedEpoch: int,
    seq: int,
    mode: WorkerMode,
    src: int
  ): bool =
    if (not(link.connected) or link.epoch != observedEpoch or seq <= 0) false
    else
      match entry {
        | AbsentEntry => true
        | Entry(e) =>
            if (seq < e.maxSeq) false
            else if (seq == e.maxSeq and mode != e.mode) false
            else
              match e.leases.get(src) {
                | NoLease => true
                | ActiveLease(l) => seq > l.leaseSeq
              }
      }

  // 应用一条已通过校验的 presence 公告: 相同序号不延长 lastAcceptedAt.
  pure def applyObserve(
    entry: RegistryEntry,
    observedEpoch: int,
    seq: int,
    mode: WorkerMode,
    src: int,
    currentTime: int
  ): RegistryEntry =
    match entry {
      | AbsentEntry =>
          Entry({
            mode: mode,
            maxSeq: seq,
            lastAcceptedAt: currentTime,
            leases: SOURCES.mapBy(s =>
              if (s == src)
                ActiveLease({ epoch: observedEpoch, receivedAt: currentTime, leaseSeq: seq })
              else NoLease
            ),
          })
      | Entry(e) => {
          val nextReceived = if (seq > e.maxSeq) currentTime else e.lastAcceptedAt
          Entry({
            mode: mode,
            maxSeq: seq,
            lastAcceptedAt: nextReceived,
            leases: e.leases.put(
              src,
              ActiveLease({ epoch: observedEpoch, receivedAt: nextReceived, leaseSeq: seq })
            ),
          })
        }
    }

  // 清理超过 4x 保留期的单来源租约; 若某 Worker 所有租约均已清空则移除其条目.
  pure def expireEntry(entry: RegistryEntry, currentTime: int): RegistryEntry =
    match entry {
      | AbsentEntry => AbsentEntry
      | Entry(e) => {
          val nextLeases = SOURCES.mapBy(s =>
            match e.leases.get(s) {
              | NoLease => NoLease
              | ActiveLease(l) =>
                  if (currentTime - l.receivedAt < RETENTION) ActiveLease(l) else NoLease
            }
          )
          val anyLeft = SOURCES.exists(s => nextLeases.get(s) != NoLease)
          if (anyLeft) Entry({ ...e, leases: nextLeases }) else AbsentEntry
        }
    }

  // --- 状态机变量与辅助历史 ---

  var links: int -> LinkState
  var entries: int -> RegistryEntry
  var now: int
  var lastSelection: SelectionOutcome
  // 跟踪在条目未被清理期间的最小应有 seq 与曾见过的非 Ready 最高 seq.
  var retainedMinSeq: int -> int
  var retainedNonReadyMaxSeq: int -> int

  action init: bool = all {
    links' = SOURCES.mapBy(_ => { epoch: 1, connected: true }),
    entries' = WORKERS.mapBy(_ => AbsentEntry),
    now' = 0,
    lastSelection' = NoneSelected,
    retainedMinSeq' = WORKERS.mapBy(_ => 0),
    retainedNonReadyMaxSeq' = WORKERS.mapBy(_ => 0),
  }

  // 观察 Worker 公告 (允许旧 epoch 或旧 seq 到达以验证拒绝与防复活).
  action observePresence(
    src: int,
    observedEpoch: int,
    w: int,
    seq: int,
    mode: WorkerMode
  ): bool = {
    val link = links.get(src)
    val oldEntry = entries.get(w)
    val accepted = canObserve(link, oldEntry, observedEpoch, seq, mode, src)
    val nextEntry =
      if (accepted) applyObserve(oldEntry, observedEpoch, seq, mode, src, now)
      else oldEntry
    val nextNonReady =
      if (accepted) {
        if (mode == ModeOffline or mode == ModeDraining) seq
        else retainedNonReadyMaxSeq.get(w)
      } else retainedNonReadyMaxSeq.get(w)
    all {
      entries' = entries.put(w, nextEntry),
      links' = links,
      now' = now,
      lastSelection' = NoneSelected,
      retainedMinSeq' =
        if (accepted) retainedMinSeq.put(w, seq)
        else retainedMinSeq,
      retainedNonReadyMaxSeq' = retainedNonReadyMaxSeq.put(w, nextNonReady),
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
      retainedMinSeq' = retainedMinSeq,
      retainedNonReadyMaxSeq' = retainedNonReadyMaxSeq,
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
      retainedMinSeq' = retainedMinSeq,
      retainedNonReadyMaxSeq' = retainedNonReadyMaxSeq,
    }
  }

  action advanceClock: bool = {
    val nextNow = now + 1
    val nextEntries = WORKERS.mapBy(w => expireEntry(entries.get(w), nextNow))
    all {
      now < MAX_NOW,
      now' = nextNow,
      entries' = nextEntries,
      links' = links,
      lastSelection' = NoneSelected,
      retainedMinSeq' = WORKERS.mapBy(w =>
        if (nextEntries.get(w) == AbsentEntry) 0 else retainedMinSeq.get(w)
      ),
      retainedNonReadyMaxSeq' = WORKERS.mapBy(w =>
        if (nextEntries.get(w) == AbsentEntry) 0 else retainedNonReadyMaxSeq.get(w)
      ),
    }
  }

  // 在当前视图上执行一次选择观测 (常可触发, 避免时钟到达上界后死锁).
  action selectAction(target: TargetSelection): bool = all {
    lastSelection' = evalSelect(links, entries, now, target),
    links' = links,
    entries' = entries,
    now' = now,
    retainedMinSeq' = retainedMinSeq,
    retainedNonReadyMaxSeq' = retainedNonReadyMaxSeq,
  }

  action step: bool = {
    nondet src = SOURCES.oneOf()
    nondet w = WORKERS.oneOf()
    nondet seq = SEQS.oneOf()
    nondet ep = 1.to(MAX_EPOCHS).oneOf()
    nondet mode = Set(ModeReady, ModeDraining, ModeOffline).oneOf()
    nondet target = Set(AnyWorker, ExactWorker(1)).oneOf()
    any {
      observePresence(src, ep, w, seq, mode),
      disconnectLink(src),
      reconnectLink(src),
      advanceClock,
      selectAction(target),
    }
  }

  // --- 安全不变量 (对应 05-roles.md 中的 INV-REG-*) ---

  // INV-REG-01: 任意被判定为可路由或被选中的 (worker, source) 必须满足连接在线, 世代匹配, 未超时且状态为 Ready.
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

  // INV-REG-02: 保留期内 sequence 单调不降, 非 Ready 记录不会被更旧或相同 seq 的 Ready 复活.
  val inv_reg_02_monotonic_seq_blocks_stale_revival: bool =
    WORKERS.forall(w =>
      match entries.get(w) {
        | AbsentEntry => true
        | Entry(e) => and {
            e.maxSeq >= retainedMinSeq.get(w),
            (e.mode == ModeOffline or e.mode == ModeDraining) implies
              SOURCES.forall(src => not(isRoutable(links, entries, now, w, src))),
            (retainedNonReadyMaxSeq.get(w) > 0 and e.maxSeq <= retainedNonReadyMaxSeq.get(w)) implies
              SOURCES.forall(src => not(isRoutable(links, entries, now, w, src))),
          }
      }
    )

  // INV-REG-03: 多传输来源租约相互隔离, 任一来源断线不会使其他保持在线且有效世代上的来源失效.
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
    inv_reg_02_monotonic_seq_blocks_stale_revival,
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
