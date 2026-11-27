// 目标选定(Target Any / Exact)、定向派发(Dispatch / DispatchReply)与多 Attempt 演进模型.
module routing {
  type TargetKind =
    | TargetAnyKind
    | TargetExactKind(int)

  type OpState =
    | OpUncreated
    | OpIdle
    | OpAwaiting({ attemptId: int, lockedWorker: int, target: TargetKind })
    | OpClosed

  type DispatchMsg = {
    attemptId: int,
    worker: int,
  }

  type ReplyKind =
    | ReplyReport(int)
    | ReplyFailed

  type DispatchReplyMsg = {
    worker: int,
    attemptId: int,
    kind: ReplyKind,
  }

  type IssuedRecord = {
    attemptId: int,
    worker: int,
    target: TargetKind,
  }

  type AuthenticReport = {
    reportId: int,
    attemptId: int,
    worker: int,
  }

  pure val WORKERS = Set(1, 2)
  pure val ATTEMPT_IDS = Set(1, 2)
  pure val BOGUS_ATTEMPT = 9
  pure val BOGUS_REPORT = 9

  var workerRoutable: int -> bool
  var opState: OpState
  var usedAttempts: Set[int]
  var issuedHistory: List[IssuedRecord]
  var acceptedCount: int
  var preMintRejected: bool
  var unresolvedWaitEnded: bool
  var injectedBogus: bool
  var inFlightDispatches: Set[DispatchMsg]
  var inFlightReplies: Set[DispatchReplyMsg]
  var authenticReports: Set[AuthenticReport]

  action init: bool = all {
    workerRoutable' = WORKERS.mapBy(_ => true),
    opState' = OpUncreated,
    usedAttempts' = Set(),
    issuedHistory' = List(),
    acceptedCount' = 0,
    preMintRejected' = false,
    unresolvedWaitEnded' = false,
    injectedBogus' = false,
    inFlightDispatches' = Set(),
    inFlightReplies' = Set(),
    authenticReports' = Set(),
  }

  action createOperation: bool = all {
    opState == OpUncreated,
    opState' = OpIdle,
    workerRoutable' = workerRoutable,
    usedAttempts' = usedAttempts,
    issuedHistory' = issuedHistory,
    acceptedCount' = acceptedCount,
    preMintRejected' = preMintRejected,
    unresolvedWaitEnded' = unresolvedWaitEnded,
    injectedBogus' = injectedBogus,
    inFlightDispatches' = inFlightDispatches,
    inFlightReplies' = inFlightReplies,
    authenticReports' = authenticReports,
  }

  action toggleWorkerRoutable(target: int): bool = all {
    workerRoutable' = workerRoutable.put(target, not(workerRoutable.get(target))),
    opState' = opState,
    usedAttempts' = usedAttempts,
    issuedHistory' = issuedHistory,
    acceptedCount' = acceptedCount,
    preMintRejected' = preMintRejected,
    unresolvedWaitEnded' = unresolvedWaitEnded,
    injectedBogus' = injectedBogus,
    inFlightDispatches' = inFlightDispatches,
    inFlightReplies' = inFlightReplies,
    authenticReports' = authenticReports,
  }

  // Exact 派发: 目标不可路由时直接拒绝本次派发, 不签发 Attempt, 保持 OpIdle 且绝不回退.
  action dispatchExact(attemptId: int, target: int): bool = all {
    opState == OpIdle,
    not(usedAttempts.contains(attemptId)),
    if (workerRoutable.get(target)) all {
      opState' = OpAwaiting({ attemptId: attemptId, lockedWorker: target, target: TargetExactKind(target) }),
      usedAttempts' = usedAttempts.union(Set(attemptId)),
      issuedHistory' = issuedHistory.append({ attemptId: attemptId, worker: target, target: TargetExactKind(target) }),
      preMintRejected' = preMintRejected,
      inFlightDispatches' = inFlightDispatches.union(Set({ attemptId: attemptId, worker: target })),
    } else all {
      opState' = OpIdle,
      usedAttempts' = usedAttempts,
      issuedHistory' = issuedHistory,
      preMintRejected' = true,
      inFlightDispatches' = inFlightDispatches,
    },
    workerRoutable' = workerRoutable,
    acceptedCount' = acceptedCount,
    unresolvedWaitEnded' = unresolvedWaitEnded,
    injectedBogus' = injectedBogus,
    inFlightReplies' = inFlightReplies,
    authenticReports' = authenticReports,
  }

  // Any 派发: 从当前可路由候选中任选其一, 无候选时不签发 Attempt 并保持 OpIdle.
  action dispatchAny(attemptId: int): bool = all {
    opState == OpIdle,
    not(usedAttempts.contains(attemptId)),
    if (WORKERS.exists(w => workerRoutable.get(w))) {
      nondet picked = WORKERS.filter(w => workerRoutable.get(w)).oneOf()
      all {
        opState' = OpAwaiting({ attemptId: attemptId, lockedWorker: picked, target: TargetAnyKind }),
        usedAttempts' = usedAttempts.union(Set(attemptId)),
        issuedHistory' = issuedHistory.append({ attemptId: attemptId, worker: picked, target: TargetAnyKind }),
        preMintRejected' = preMintRejected,
        inFlightDispatches' = inFlightDispatches.union(Set({ attemptId: attemptId, worker: picked })),
      }
    } else all {
      opState' = OpIdle,
      usedAttempts' = usedAttempts,
      issuedHistory' = issuedHistory,
      preMintRejected' = true,
      inFlightDispatches' = inFlightDispatches,
    },
    workerRoutable' = workerRoutable,
    acceptedCount' = acceptedCount,
    unresolvedWaitEnded' = unresolvedWaitEnded,
    injectedBogus' = injectedBogus,
    inFlightReplies' = inFlightReplies,
    authenticReports' = authenticReports,
  }

  // Worker 正常执行并返回携带自身身份与 Report 的 DispatchReply.
  action workerReplyReport(msg: DispatchMsg): bool = {
    val reply = {
      worker: msg.worker,
      attemptId: msg.attemptId,
      kind: ReplyReport(msg.attemptId),
    }
    val auth = {
      reportId: msg.attemptId,
      attemptId: msg.attemptId,
      worker: msg.worker,
    }
    all {
      inFlightDispatches.contains(msg),
      inFlightDispatches' = inFlightDispatches.exclude(Set(msg)),
      inFlightReplies' = inFlightReplies.union(Set(reply)),
      authenticReports' = authenticReports.union(Set(auth)),
      workerRoutable' = workerRoutable,
      opState' = opState,
      usedAttempts' = usedAttempts,
      issuedHistory' = issuedHistory,
      acceptedCount' = acceptedCount,
      preMintRejected' = preMintRejected,
      unresolvedWaitEnded' = unresolvedWaitEnded,
      injectedBogus' = injectedBogus,
    }
  }

  // Worker 准入拒收(如 Draining/能力不符)或执行期异常: 返回携带自身身份的拒绝/失败通知, 不伪造 Report.
  action workerReplyFailed(msg: DispatchMsg): bool = {
    val reply = {
      worker: msg.worker,
      attemptId: msg.attemptId,
      kind: ReplyFailed,
    }
    all {
      inFlightDispatches.contains(msg),
      inFlightDispatches' = inFlightDispatches.exclude(Set(msg)),
      inFlightReplies' = inFlightReplies.union(Set(reply)),
      workerRoutable' = workerRoutable,
      opState' = opState,
      usedAttempts' = usedAttempts,
      issuedHistory' = issuedHistory,
      acceptedCount' = acceptedCount,
      preMintRejected' = preMintRejected,
      unresolvedWaitEnded' = unresolvedWaitEnded,
      injectedBogus' = injectedBogus,
      authenticReports' = authenticReports,
    }
  }

  // 注入错配来源 Worker 或错配 Attempt 的非法 Report 响应.
  action injectInvalidReply(spoofWorker: bool): bool = {
    val awaitingOk = match opState {
      | OpAwaiting(_) => not(injectedBogus)
      | _ => false
    }
    val bogus = match opState {
      | OpAwaiting(a) =>
          if (spoofWorker) {
            worker: if (a.lockedWorker == 1) 2 else 1,
            attemptId: a.attemptId,
            kind: ReplyReport(BOGUS_REPORT),
          } else {
            worker: a.lockedWorker,
            attemptId: BOGUS_ATTEMPT,
            kind: ReplyReport(BOGUS_REPORT),
          }
      | _ => { worker: 1, attemptId: BOGUS_ATTEMPT, kind: ReplyReport(BOGUS_REPORT) }
    }
    all {
      awaitingOk,
      injectedBogus' = true,
      inFlightReplies' = inFlightReplies.union(Set(bogus)),
      workerRoutable' = workerRoutable,
      opState' = opState,
      usedAttempts' = usedAttempts,
      issuedHistory' = issuedHistory,
      acceptedCount' = acceptedCount,
      preMintRejected' = preMintRejected,
      unresolvedWaitEnded' = unresolvedWaitEnded,
      inFlightDispatches' = inFlightDispatches,
      authenticReports' = authenticReports,
    }
  }

  // Authority 等待超时或断线: 结束本地等待但保持 OpAwaiting, 不自动重试或关闭.
  action authorityTimeout: bool = {
    val canTimeout = match opState {
      | OpAwaiting(_) => true
      | _ => false
    }
    all {
      canTimeout,
      unresolvedWaitEnded' = true,
      workerRoutable' = workerRoutable,
      opState' = opState,
      usedAttempts' = usedAttempts,
      issuedHistory' = issuedHistory,
      acceptedCount' = acceptedCount,
      preMintRejected' = preMintRejected,
      injectedBogus' = injectedBogus,
      inFlightDispatches' = inFlightDispatches,
      inFlightReplies' = inFlightReplies,
      authenticReports' = authenticReports,
    }
  }

  // Authority 验收 DispatchReply:
  // 仅当携带 Report 且回复方 Worker 与锁定目标及 Attempt 均匹配时回到 OpIdle(不要求当前仍可路由).
  action authorityReceiveReply(reply: DispatchReplyMsg): bool = {
    val validReport = match opState {
      | OpAwaiting(a) =>
          reply.worker == a.lockedWorker and reply.attemptId == a.attemptId and
            match reply.kind {
              | ReplyReport(_) => true
              | ReplyFailed => false
            }
      | _ => false
    }
    val matchedFailed = match opState {
      | OpAwaiting(a) =>
          reply.worker == a.lockedWorker and reply.attemptId == a.attemptId and reply.kind == ReplyFailed
      | _ => false
    }
    val nextOpState = if (validReport) OpIdle else opState
    val nextAccepted = if (validReport) acceptedCount + 1 else acceptedCount
    all {
      inFlightReplies.contains(reply),
      inFlightReplies' = inFlightReplies.exclude(Set(reply)),
      opState' = nextOpState,
      acceptedCount' = nextAccepted,
      unresolvedWaitEnded' = (unresolvedWaitEnded or matchedFailed),
      workerRoutable' = workerRoutable,
      usedAttempts' = usedAttempts,
      issuedHistory' = issuedHistory,
      preMintRejected' = preMintRejected,
      injectedBogus' = injectedBogus,
      inFlightDispatches' = inFlightDispatches,
      authenticReports' = authenticReports,
    }
  }

  // 仅处于 OpIdle(或已 OpClosed)时允许显式关闭 Operation.
  action closeOperation: bool = {
    val canClose = (opState == OpIdle or opState == OpClosed)
    all {
      canClose,
      opState' = OpClosed,
      workerRoutable' = workerRoutable,
      usedAttempts' = usedAttempts,
      issuedHistory' = issuedHistory,
      acceptedCount' = acceptedCount,
      preMintRejected' = preMintRejected,
      unresolvedWaitEnded' = unresolvedWaitEnded,
      injectedBogus' = injectedBogus,
      inFlightDispatches' = inFlightDispatches,
      inFlightReplies' = inFlightReplies,
      authenticReports' = authenticReports,
    }
  }

  action step: bool = any {
    createOperation,
    {
      nondet target = WORKERS.oneOf()
      toggleWorkerRoutable(target)
    },
    {
      nondet attemptId = ATTEMPT_IDS.oneOf()
      nondet target = WORKERS.oneOf()
      dispatchExact(attemptId, target)
    },
    {
      nondet attemptId = ATTEMPT_IDS.oneOf()
      dispatchAny(attemptId)
    },
    {
      nondet msg = inFlightDispatches.oneOf()
      all {
        inFlightDispatches.size() > 0,
        workerReplyReport(msg),
      }
    },
    {
      nondet msg = inFlightDispatches.oneOf()
      all {
        inFlightDispatches.size() > 0,
        workerReplyFailed(msg),
      }
    },
    {
      nondet spoofWorker = Set(true, false).oneOf()
      injectInvalidReply(spoofWorker)
    },
    authorityTimeout,
    {
      nondet reply = inFlightReplies.oneOf()
      all {
        inFlightReplies.size() > 0,
        authorityReceiveReply(reply),
      }
    },
    closeOperation,
  }

  // Exact 目标绝不回退到其他 Worker.
  val exactNeverFallsBack: bool = and {
    issuedHistory.foldl(true, (ok, h) =>
      ok and match h.target {
        | TargetAnyKind => true
        | TargetExactKind(w) => h.worker == w
      }
    ),
    match opState {
      | OpAwaiting(a) =>
          match a.target {
            | TargetAnyKind => true
            | TargetExactKind(w) => a.lockedWorker == w
          }
      | _ => true
    },
  }

  // 选路拒绝不签发 Attempt; 只有验收完最新 Attempt 后才可回到 OpIdle 或进入 OpClosed.
  val attemptAccountingConsistent: bool = and {
    usedAttempts.size() == issuedHistory.length(),
    acceptedCount <= issuedHistory.length(),
    (opState == OpUncreated) implies (issuedHistory.length() == 0),
    (opState == OpIdle or opState == OpClosed) implies (issuedHistory.length() == acceptedCount),
    match opState {
      | OpAwaiting(a) =>
          issuedHistory.length() == acceptedCount + 1 and
            issuedHistory[issuedHistory.length() - 1].attemptId == a.attemptId and
            issuedHistory[issuedHistory.length() - 1].worker == a.lockedWorker
      | _ => true
    },
  }

  // 每一次已验收的 Attempt 都必定对应锁定 Worker 真实产出的 Report(错配来源或失败通知绝不能解除等待).
  val acceptedAttemptsRequireAuthenticLockedReport: bool =
    Set(0, 1).forall(idx =>
      if (idx >= acceptedCount) true
      else {
        val h = issuedHistory[idx]
        authenticReports.contains({
          reportId: h.attemptId,
          attemptId: h.attemptId,
          worker: h.worker,
        })
      }
    )

  val invariants: bool = and {
    exactNeverFallsBack,
    attemptAccountingConsistent,
    acceptedAttemptsRequireAuthenticLockedReport,
  }

  val witnessMultiAttemptCompleted: bool =
    opState == OpClosed and acceptedCount == 2

  val witnessExactRejectedWithoutMinting: bool =
    preMintRejected and opState == OpIdle

  val witnessFailedOrTimeoutKeepsAwaiting: bool =
    unresolvedWaitEnded and
      match opState {
        | OpAwaiting(_) => true
        | _ => false
      }

  val witnessLateReportAcceptedWhileUnroutable: bool =
    acceptedCount >= 1 and issuedHistory.length() >= 1 and not(workerRoutable.get(issuedHistory[0].worker))
}
