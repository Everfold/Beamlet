module operation {
  /// Operation 身份与不可变请求的投影, 生命周期由状态机描述.
  type Operation[o, p] = { id: o, request: p }

  /// 用外层提供的身份和内容构造请求.
  pure def makeOperation(id: o, request: p): Operation[o, p] =
    { id: id, request: request }
}
