package io.github.mcdev.core.mixinextras
import java.nio.file.Files
import java.nio.file.Path
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.objectweb.asm.ClassReader
import org.objectweb.asm.tree.ClassNode

class LocalFlowOracle {
 @Test fun debugFramesPreserveLoopExceptionMergeWideConstructorAndUnicodeFacts() = check("debug")
 @Test fun absentDebugFramesPreserveLoopExceptionMergeWideAndConstructorFacts() = check("no-debug")
 private fun check(mode:String){
  val bytes=Files.readAllBytes(Path.of("build/locals-additional-oracle/$mode/LocalFlowSamples.class"))
  val clazz=ClassNode();ClassReader(bytes).accept(clazz,ClassReader.SKIP_FRAMES)
  for(method in clazz.methods){
   val count=method.instructions.toArray().count{it.opcode>=0}
   for(index in 0 until count){
    val result=assertIs<LocalCaptureResult.Success>(LocalCaptureExtractor.extract(bytes,method.name,method.desc,setOf(index)))
    val snapshot=result.snapshots.single();assertEquals(index,snapshot.instructionOccurrenceIndex)
    assertEquals(snapshot.candidates.map{it.slotIndex}.sorted(),snapshot.candidates.map{it.slotIndex})
    assertTrue(snapshot.candidates.none{it.descriptor=="V"})
   }
  }
 }
}
