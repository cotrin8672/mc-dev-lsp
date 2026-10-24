package oracle
object FacadeCapture {
 fun sources(provider:()->Sequence<String>):Sequence<String> {
  Capture.read("Share.sources",emptyList<String>(),true)
  return provider().map { Capture.read("Share.source",emptyList<String>(),it) }
 }
}
