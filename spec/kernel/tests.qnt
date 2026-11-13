module tests {
  import operation.* from "./operation"
  import attempt.* from "./attempt"
  import report.* from "./report"
  import state_machine.* from "./state_machine"

  // 有限身份只用于检查, 不约束实际实现的身份格式.
  pure val currentOperation = makeOperation(1, "request")
  pure val attemptIds = Set(1, 2, 3)
  pure val operationIds = Set(1, 2)
  pure val states: Set[State[int]] =
    Set(Ready, Closed).union(attemptIds.map(id => AwaitingReport(id)))

  var state: State[int]
  var previous: State[int]
  var outcome: Outcome[int, int]
  var used: Set[int]

  /// 初始化一个 Operation 的外部状态及检查记录.
  action init = all {
    state' = Ready,
    previous' = Ready,
    outcome' = Updated(Ready),
    used' = Set(),
  }

  /// 外层应用转换结果, used 仅用于模拟新身份约束.
  action apply(result: Outcome[int, int]): bool = all {
    previous' = state,
    state' = nextState(state, result),
    outcome' = result,
    used' = match result {
      | Created(created) => used.union(Set(created.attempt.id))
      | _ => used
    },
  }

  /// 尝试创建 Attempt, Ready 时由外层提供未使用的身份.
  action create(id: int): bool = all {
    state != Ready or not(used.contains(id)),
    apply(createAttempt(state, currentOperation.id, id)),
  }

  /// 提交报告, 内容不参与关联校验.
  action accept(operationId: int, attemptId: int): bool =
    apply(acceptReport(
      state, currentOperation.id, makeReport(1, operationId, attemptId, "result")
    ))

  /// 在有限身份集合中探索三个动作及其拒绝结果.
  action step = {
    nondet id = attemptIds.oneOf()
    nondet operationId = operationIds.oneOf()
    any {
      create(id),
      accept(operationId, id),
      apply(close(state)),
    }
  }

  /// 失败不改变生命周期状态.
  val rejectionKeepsState = match outcome {
    | Rejected(_) => state == previous
    | _ => true
  }

  /// 已关闭的 Operation 不会重新打开.
  val closedStaysClosed = previous != Closed or state == Closed

  /// 创建只发生在 Ready, 且输出关联和等待状态一致.
  val creationIsConsistent = match outcome {
    | Created(created) => and {
        previous == Ready,
        created.attempt.operation_id == currentOperation.id,
        state == AwaitingReport(created.attempt.id),
      }
    | _ => true
  }

  /// 汇总模拟与模型检查使用的性质.
  val invariants = rejectionKeepsState and closedStaysClosed and creationIsConsistent

  /// 同内容的不同身份仍然是不同请求.
  run distinctOperationsTest = assert(
    makeOperation(1, "request") != makeOperation(2, "request")
  )

  /// 枚举当前状态与身份组合, 检查创建的成功条件和错误.
  run creationRulesTest = assert(states.forall(s => attemptIds.forall(id =>
    createAttempt(s, 1, id) == match s {
      | Ready => Created({ attempt: makeAttempt(1, id), state: AwaitingReport(id) })
      | AwaitingReport(_) => Rejected(AttemptPending)
      | Closed => Rejected(OperationClosed)
    }
  )))

  /// 枚举报告关联, 检查错误优先级与接受条件.
  run reportRulesTest = assert(states.forall(s =>
    operationIds.forall(o => attemptIds.forall(a =>
      acceptReport(s, 1, makeReport(1, o, a, "result")) == match s {
        | Ready => Rejected(NoPendingAttempt)
        | Closed => Rejected(OperationClosed)
        | AwaitingReport(pending) =>
            if (o != 1) Rejected(OperationMismatch)
            else if (a != pending) Rejected(AttemptMismatch)
            else Updated(Ready)
      }
    ))
  ))

  /// 枚举关闭规则, 包括重复关闭.
  run closeRulesTest = assert(states.forall(s =>
    close(s) == match s {
      | Ready => Updated(Closed)
      | Closed => Updated(Closed)
      | AwaitingReport(_) => Rejected(AttemptPending)
    }
  ))

  /// 接受报告后创建新 Attempt, 旧报告不能解除新的等待.
  run lifecycleTest =
    init
      .then(create(1))
      .then(accept(1, 1))
      .then(create(2))
      .then(accept(1, 1))
      .expect(state == AwaitingReport(2) and outcome == Rejected(AttemptMismatch))
      .then(accept(1, 2))
      .then(apply(close(state)))
      .then(apply(close(state)))
      .then(apply(createAttempt(state, 1, 3)))
      .expect(state == Closed and outcome == Rejected(OperationClosed))
}
