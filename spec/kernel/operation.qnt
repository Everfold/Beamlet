module operation {
  /// 不可变执行请求, o 为身份类型, p 为请求内容类型.
  type Operation[o, p] = { id: o, request: p }

  /// 用外层提供的身份和内容构造请求.
  pure def makeOperation(id: o, request: p): Operation[o, p] =
    { id: id, request: request }
}
