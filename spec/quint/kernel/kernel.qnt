// L1 内核有界模型探索: 验证单 Operation 状态机不变量与可达性见证.
module kernel {
  import primitives.* from "./primitives"
  import state_machine.* from "./state_machine"

  pure val CURRENT_OP = makeOperation(1, "request")
  pure val OPERATION_IDS = Set(1, 2)
  pure val ATTEMPT_IDS = Set(1, 2, 3)

  type ReportInput =
    | NoReport
    | SubmittedReport({ operation_id: int, attempt_id: int })

  var state: State[int]
  var previous: State[int]
  var outcome: KernelOutcome[int, int]
  var used: Set[int]
  var reportInput: ReportInput

  action init: bool = all {
    state' = Ready,
    previous' = Ready,
    outcome' = Updated(Ready),
    used' = Set(),
    reportInput' = NoReport,
  }

  action applyOutcome(
    result: KernelOutcome[int, int],
    input: ReportInput
  ): bool = all {
    previous' = state,
    state' = nextState(state, result),
    outcome' = result,
    reportInput' = input,
    used' = match result {
      | Created(created) => used.union(Set(created.attempt.id))
      | _ => used
    },
  }

  // 仅在 Ready 时要求外层提供未使用的 Attempt 身份; 非 Ready 时模拟非法重复触发.
  action createAction(attemptId: int): bool = all {
    state != Ready or not(used.contains(attemptId)),
    applyOutcome(createAttempt(state, CURRENT_OP.id, attemptId), NoReport),
  }

  action acceptAction(opId: int, attemptId: int): bool =
    applyOutcome(
      acceptReport(
        state,
        CURRENT_OP.id,
        makeReport(1, opId, attemptId, "result")
      ),
      SubmittedReport({ operation_id: opId, attempt_id: attemptId })
    )

  action closeAction: bool =
    applyOutcome(close(state), NoReport)

  action step: bool = {
    nondet attemptId = ATTEMPT_IDS.oneOf()
    nondet opId = OPERATION_IDS.oneOf()
    any {
      createAction(attemptId),
      acceptAction(opId, attemptId),
      closeAction,
    }
  }

  // --- 安全不变量 (对应 02-kernel.md 中的 INV-KRN-*) ---

  // INV-KRN-01: 任何被拒绝的动作都不得改变生命周期状态.
  val inv_krn_01_rejection_keeps_state: bool =
    match outcome {
      | Rejected(_) => state == previous
      | _ => true
    }

  // INV-KRN-02: 一旦进入 Closed, 后续任何转换都不得重新打开 Operation.
  val inv_krn_02_closed_stays_closed: bool =
    previous != Closed or state == Closed

  // INV-KRN-03: 创建 Attempt 仅在 Ready 发生, 且返回授权与等待状态完全一致.
  val inv_krn_03_creation_is_consistent: bool =
    match outcome {
      | Created(created) => and {
          previous == Ready,
          created.attempt.operation_id == CURRENT_OP.id,
          state == AwaitingReport(created.attempt.id),
        }
      | _ => true
    }

  // INV-KRN-04: 接受 Report 仅在 AwaitingReport 且两个引用身份精确匹配时发生, 并转回 Ready.
  val inv_krn_04_report_acceptance_is_valid: bool =
    match reportInput {
      | NoReport => true
      | SubmittedReport(ev) =>
          match outcome {
            | Rejected(_) => true
            | Created(_) => false
            | Updated(newState) =>
                match previous {
                  | AwaitingReport(pending) => and {
                      ev.operation_id == CURRENT_OP.id,
                      ev.attempt_id == pending,
                      newState == Ready,
                      state == Ready,
                    }
                  | _ => false
                }
          }
    }

  val invariants: bool = and {
    inv_krn_01_rejection_keeps_state,
    inv_krn_02_closed_stays_closed,
    inv_krn_03_creation_is_consistent,
    inv_krn_04_report_acceptance_is_valid,
  }

  // --- 可达性见证 (Witnesses) ---

  val witnessAttemptCreated: bool =
    match outcome {
      | Created(_) => true
      | _ => false
    }

  val witnessReportAccepted: bool =
    match reportInput {
      | NoReport => false
      | SubmittedReport(_) =>
          match outcome {
            | Updated(newState) => newState == Ready
            | _ => false
          }
    }

  val witnessClosed: bool = state == Closed

  val witnessRejectedMismatch: bool =
    match outcome {
      | Rejected(err) => err == OperationMismatch or err == AttemptMismatch
      | _ => false
    }
}
