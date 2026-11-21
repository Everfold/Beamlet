// L2 在线台账场景测试套件.
module ledger_test {
  import primitives.* from "../kernel/primitives"
  import state_machine.* from "../kernel/state_machine"
  import ledger.* from "./ledger"

  // 重复创建 Operation 拒绝, 且不影响原 Operation 状态.
  run duplicateCreateOpTest =
    init
      .then(createOpAction(1))
      .expect(lastOutcome == LedgerCreatedOp(1))
      .then(createAttemptAction(1, 1))
      .then(createOpAction(1))
      .expect(lastOutcome == LedgerErr(OperationExists) and isPendingOperation(ledger.entries.get(1)))

  // 跨 Operation 复用已签发的 Attempt 身份必须被拒绝, 且拒绝后被拒身份仍不影响合法重用检验.
  run crossOpAttemptReuseTest =
    init
      .then(createOpAction(1))
      .then(createOpAction(2))
      .then(createAttemptAction(1, 1))
      .expect(lastOutcome == LedgerIssuedAttempt({ opId: 1, attemptId: 1 }))
      .then(createAttemptAction(2, 1))
      .expect(lastOutcome == LedgerErr(AttemptIdReused) and ledger.entries.get(2) == Managed(Ready))
      .then(createAttemptAction(2, 2))
      .expect(lastOutcome == LedgerIssuedAttempt({ opId: 2, attemptId: 2 }))

  // 状态非法(例如 AwaitingReport)时被拒绝的 Attempt 身份不得被标记为已使用.
  run rejectedCreateDoesNotPolluteUsedTest =
    init
      .then(createOpAction(1))
      .then(createOpAction(2))
      .then(createAttemptAction(1, 1))
      .then(createAttemptAction(1, 2))
      .expect(lastOutcome == LedgerErr(AttemptPendingErr) and not(ledger.usedAttempts.contains(2)))
      .then(createAttemptAction(2, 2))
      .expect(lastOutcome == LedgerIssuedAttempt({ opId: 2, attemptId: 2 }))

  // 错误的 Report 不解除等待状态, 正确 Report 后支持重新签发新 Attempt 再关闭.
  run mismatchKeepsPendingAndRetryTest =
    init
      .then(createOpAction(1))
      .then(createAttemptAction(1, 1))
      .then(acceptReportAction(1, 2))
      .expect(lastOutcome == LedgerErr(ReportMismatch) and pendingAttemptOf(ledger.entries.get(1)) == 1)
      .then(acceptReportAction(1, 1))
      .expect(lastOutcome == LedgerAcceptedReport({ opId: 1, attemptId: 1 }))
      .then(createAttemptAction(1, 2))
      .then(acceptReportAction(1, 2))
      .then(closeOpAction(1))
      .then(closeOpAction(1))
      .expect(lastOutcome == LedgerClosedOp(1) and ledger.entries.get(1) == Managed(Closed))
}
