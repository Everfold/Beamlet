// Ledger 在线执行台账: 管理多个 Operation 并强制 Attempt 身份全局不重复.
// 不依赖角色, 传输或外部存储.
module ledger {
  import primitives.* from "../kernel/primitives"
  import state_machine.* from "../kernel/state_machine"

  type LedgerSlot =
    | Uncreated
    | Managed(State[int])

  // 台账转换被拒绝的原因分类.
  type LedgerError =
    | OperationExists
    | OperationNotFound
    | AttemptPendingErr
    | NoPendingAttemptErr
    | OperationClosedErr
    | AttemptIdReused
    | ReportMismatch

  type LedgerState = {
    entries: int -> LedgerSlot,
    usedAttempts: Set[int],
  }

  type LedgerEvent =
    | LedgerInit
    | LedgerCreatedOp(int)
    | LedgerIssuedAttempt({ opId: int, attemptId: int })
    | LedgerAcceptedReport({ opId: int, attemptId: int })
    | LedgerClosedOp(int)
    | LedgerErr(LedgerError)

  type LedgerResult = {
    ledger: LedgerState,
    event: LedgerEvent,
  }

  pure val OPERATION_IDS = Set(1, 2)
  pure val ATTEMPT_IDS = Set(1, 2, 3)

  // 创建 Idle Operation; 身份已存在则拒绝且不覆盖原记录.
  pure def ledgerCreateOperation(state: LedgerState, opId: int): LedgerResult =
    match state.entries.get(opId) {
      | Uncreated =>
          {
            ledger: {
              ...state,
              entries: state.entries.put(opId, Managed(Idle)),
            },
            event: LedgerCreatedOp(opId),
          }
      | Managed(_) =>
          {
            ledger: state,
            event: LedgerErr(OperationExists),
          }
    }

  // 为 Idle Operation 签发未使用的 Attempt; 任何拒绝均不污染 usedAttempts.
  pure def ledgerCreateAttempt(
    state: LedgerState,
    opId: int,
    attemptId: int
  ): LedgerResult =
    match state.entries.get(opId) {
      | Uncreated => { ledger: state, event: LedgerErr(OperationNotFound) }
      | Managed(opState) =>
          match opState {
            | AwaitingReport(_) =>
                { ledger: state, event: LedgerErr(AttemptPendingErr) }
            | Closed =>
                { ledger: state, event: LedgerErr(OperationClosedErr) }
            | Idle =>
                if (state.usedAttempts.contains(attemptId))
                  { ledger: state, event: LedgerErr(AttemptIdReused) }
                else
                  match createAttempt(opState, opId, attemptId) {
                    | Created(created) =>
                        {
                          ledger: {
                            entries: state.entries.put(opId, Managed(created.state)),
                            usedAttempts: state.usedAttempts.union(Set(attemptId)),
                          },
                          event: LedgerIssuedAttempt({ opId: opId, attemptId: attemptId }),
                        }
                    | Updated(_) => { ledger: state, event: LedgerErr(AttemptPendingErr) }
                    | Rejected(_) => { ledger: state, event: LedgerErr(AttemptPendingErr) }
                  }
          }
    }

  // 查找 Report.operation_id 并委托内核校验; 失配时保留原等待态.
  pure def ledgerAcceptReport(
    state: LedgerState,
    evidence: Report[int, int, int, str]
  ): LedgerResult =
    match state.entries.get(evidence.operation_id) {
      | Uncreated => { ledger: state, event: LedgerErr(OperationNotFound) }
      | Managed(opState) =>
          match opState {
            | Idle =>
                { ledger: state, event: LedgerErr(NoPendingAttemptErr) }
            | Closed =>
                { ledger: state, event: LedgerErr(OperationClosedErr) }
            | AwaitingReport(pendingAttempt) =>
                match acceptReport(opState, evidence.operation_id, evidence) {
                  | Updated(newState) =>
                      {
                        ledger: {
                          ...state,
                          entries: state.entries.put(evidence.operation_id, Managed(newState)),
                        },
                        event: LedgerAcceptedReport({
                          opId: evidence.operation_id,
                          attemptId: pendingAttempt,
                        }),
                      }
                  | Rejected(_) =>
                      { ledger: state, event: LedgerErr(ReportMismatch) }
                  | Created(_) =>
                      { ledger: state, event: LedgerErr(ReportMismatch) }
                }
          }
    }

  // 关闭 Idle 或幂等关闭 Closed Operation; 等待中时拒绝.
  pure def ledgerClose(state: LedgerState, opId: int): LedgerResult =
    match state.entries.get(opId) {
      | Uncreated => { ledger: state, event: LedgerErr(OperationNotFound) }
      | Managed(opState) =>
          match close(opState) {
            | Updated(newState) =>
                {
                  ledger: {
                    ...state,
                    entries: state.entries.put(opId, Managed(newState)),
                  },
                  event: LedgerClosedOp(opId),
                }
            | Rejected(_) =>
                { ledger: state, event: LedgerErr(AttemptPendingErr) }
            | Created(_) =>
                { ledger: state, event: LedgerErr(AttemptPendingErr) }
          }
    }

  var ledger: LedgerState
  var lastEvent: LedgerEvent
  var issuedLog: List[{ opId: int, attemptId: int }]
  var everClosed: Set[int]
  var acceptedOps: Set[int]
  var retriedOps: Set[int]

  action init: bool = all {
    ledger' = {
      entries: OPERATION_IDS.mapBy(_ => Uncreated),
      usedAttempts: Set(),
    },
    lastEvent' = LedgerInit,
    issuedLog' = List(),
    everClosed' = Set(),
    acceptedOps' = Set(),
    retriedOps' = Set(),
  }

  action applyLedger(res: LedgerResult): bool = all {
    ledger' = res.ledger,
    lastEvent' = res.event,
    issuedLog' = match res.event {
      | LedgerIssuedAttempt(entry) => issuedLog.append(entry)
      | _ => issuedLog
    },
    everClosed' = match res.event {
      | LedgerClosedOp(opId) => everClosed.union(Set(opId))
      | _ => everClosed
    },
    acceptedOps' = match res.event {
      | LedgerAcceptedReport(ev) => acceptedOps.union(Set(ev.opId))
      | _ => acceptedOps
    },
    retriedOps' = match res.event {
      | LedgerIssuedAttempt(entry) =>
          if (acceptedOps.contains(entry.opId)) retriedOps.union(Set(entry.opId))
          else retriedOps
      | _ => retriedOps
    },
  }

  action createOpAction(opId: int): bool =
    applyLedger(ledgerCreateOperation(ledger, opId))

  action createAttemptAction(opId: int, attemptId: int): bool =
    applyLedger(ledgerCreateAttempt(ledger, opId, attemptId))

  action acceptReportAction(opId: int, attemptId: int): bool =
    applyLedger(ledgerAcceptReport(ledger, makeReport(1, opId, attemptId, "ok")))

  action closeOpAction(opId: int): bool =
    applyLedger(ledgerClose(ledger, opId))

  action step: bool = {
    nondet opId = OPERATION_IDS.oneOf()
    nondet attemptId = ATTEMPT_IDS.oneOf()
    any {
      createOpAction(opId),
      createAttemptAction(opId, attemptId),
      acceptReportAction(opId, attemptId),
      closeOpAction(opId),
    }
  }

  pure def isPendingOperation(slot: LedgerSlot): bool =
    match slot {
      | Managed(s) =>
          match s {
            | AwaitingReport(_) => true
            | _ => false
          }
      | Uncreated => false
    }

  pure def pendingAttemptOf(slot: LedgerSlot): int =
    match slot {
      | Managed(s) =>
          match s {
            | AwaitingReport(a) => a
            | _ => -1
          }
      | Uncreated => -1
    }

  // 已关闭的 Operation 在线期间保持 Closed, 不被覆盖或重启.
  val closedIsMonotonic: bool =
    everClosed.forall(opId => ledger.entries.get(opId) == Managed(Closed))

  // 全局已签发 Attempt 身份彼此唯一, 且与 usedAttempts 完全一致.
  val attemptIdsAreUnique: bool = {
    val issuedSet = issuedLog.foldl(Set(), (acc, entry) => acc.union(Set(entry.attemptId)))
    and {
      issuedSet.size() == issuedLog.length(),
      issuedSet == ledger.usedAttempts,
    }
  }

  // 被拒绝的 CreateAttempt 绝不把未使用身份写入 usedAttempts.
  val rejectionDoesNotPolluteUsed: bool =
    ledger.usedAttempts.size() == issuedLog.length()

  // 等待中的 Operation 持有的 Attempt 已记录于 usedAttempts, 且互不相同.
  val pendingAttemptIsRecorded: bool = and {
    OPERATION_IDS.forall(opId => {
      val slot = ledger.entries.get(opId)
      not(isPendingOperation(slot)) or ledger.usedAttempts.contains(pendingAttemptOf(slot))
    }),
    OPERATION_IDS.forall(o1 => OPERATION_IDS.forall(o2 =>
      (o1 == o2 or not(isPendingOperation(ledger.entries.get(o1))) or not(isPendingOperation(ledger.entries.get(o2))))
        or pendingAttemptOf(ledger.entries.get(o1)) != pendingAttemptOf(ledger.entries.get(o2))
    )),
  }

  val invariants: bool = and {
    closedIsMonotonic,
    attemptIdsAreUnique,
    rejectionDoesNotPolluteUsed,
    pendingAttemptIsRecorded,
  }

  val witnessTwoOpsActive: bool =
    OPERATION_IDS.forall(opId => isPendingOperation(ledger.entries.get(opId)))

  val witnessAttemptReuseRejected: bool =
    lastEvent == LedgerErr(AttemptIdReused)

  val witnessRetryWithFreshAttempt: bool =
    retriedOps.size() > 0
}
