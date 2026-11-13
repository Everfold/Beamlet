module state_machine {
  import attempt.* from "./attempt"
  import report.* from "./report"

  /// 生命周期状态与原语数据分开.
  type State[a] = Ready | AwaitingReport(a) | Closed

  /// 与当前 Rust 接口一致的拒绝原因.
  type Error = AttemptPending | OperationClosed | NoPendingAttempt
    | OperationMismatch | AttemptMismatch

  /// 成功包含新状态, 创建成功还包含新的 Attempt.
  type Outcome[o, a] =
    Created({ attempt: Attempt[o, a], state: State[a] })
    | Updated(State[a])
    | Rejected(Error)

  /// 创建 Attempt, 新身份与当前 Operation 的绑定由外层保证.
  pure def createAttempt(state: State[a], operationId: o, attemptId: a): Outcome[o, a] =
    match state {
      | Ready => Created({
          attempt: makeAttempt(operationId, attemptId),
          state: AwaitingReport(attemptId)
        })
      | AwaitingReport(_) => Rejected(AttemptPending)
      | Closed => Rejected(OperationClosed)
    }

  /// 接受报告, 依次校验状态, Operation 和 Attempt.
  pure def acceptReport(
    state: State[a], operationId: o, evidence: Report[o, a, r, v]
  ): Outcome[o, a] =
    match state {
      | Ready => Rejected(NoPendingAttempt)
      | Closed => Rejected(OperationClosed)
      | AwaitingReport(pending) =>
          if (evidence.operation_id != operationId) Rejected(OperationMismatch)
          else if (evidence.attempt_id != pending) Rejected(AttemptMismatch)
          else Updated(Ready)
    }

  /// 结束 Operation, 允许重复关闭.
  pure def close(state: State[a]): Outcome[o, a] =
    match state {
      | Ready => Updated(Closed)
      | Closed => Updated(Closed)
      | AwaitingReport(_) => Rejected(AttemptPending)
    }

  /// 模拟外层保存结果, 拒绝时保留原状态.
  pure def nextState(state: State[a], outcome: Outcome[o, a]): State[a] =
    match outcome {
      | Created(created) => created.state
      | Updated(newState) => newState
      | Rejected(_) => state
    }
}
