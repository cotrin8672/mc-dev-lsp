package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import oracle.Capture
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Opcodes

/** Additional official-v0.0.6 differential evidence; these are not legacy test IDs. */
class EngineAdditionalOracle {
    @Test fun stackWideAndConstantKinds(){
        fun sample(name:String,descriptor:String="()I",locals:Int=0,body:(org.objectweb.asm.MethodVisitor)->Unit):ByteArray{
            val writer=ClassWriter(0);writer.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"oracle/StackWide",null,"java/lang/Object",null)
            val method=writer.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,"run",descriptor,null,null);method.visitCode();body(method);method.visitMaxs(32,locals);method.visitEnd();writer.visitEnd();return writer.toByteArray()
        }
        fun insns(method:org.objectweb.asm.MethodVisitor,vararg values:Int){for(value in values)method.visitInsn(value)}
        val samples=listOf(
            sample("dup"){insns(it,Opcodes.ICONST_2,Opcodes.DUP,Opcodes.IADD,Opcodes.IRETURN)},
            sample("dup_x1"){insns(it,Opcodes.ICONST_2,Opcodes.ICONST_3,Opcodes.DUP_X1,Opcodes.IADD,Opcodes.IADD,Opcodes.IRETURN)},
            sample("dup_x2_1"){insns(it,Opcodes.ICONST_1,Opcodes.ICONST_2,Opcodes.ICONST_3,Opcodes.DUP_X2,Opcodes.IADD,Opcodes.IADD,Opcodes.IADD,Opcodes.IRETURN)},
            sample("dup_x2_2"){insns(it,Opcodes.LCONST_0,Opcodes.ICONST_3,Opcodes.DUP_X2,Opcodes.POP,Opcodes.POP2,Opcodes.IRETURN)},
            sample("dup2_1"){insns(it,Opcodes.ICONST_1,Opcodes.ICONST_2,Opcodes.DUP2,Opcodes.IADD,Opcodes.IADD,Opcodes.IADD,Opcodes.IRETURN)},
            sample("dup2_2"){insns(it,Opcodes.LCONST_0,Opcodes.DUP2,Opcodes.LCMP,Opcodes.IRETURN)},
            sample("dup2_x1_1"){insns(it,Opcodes.ICONST_1,Opcodes.ICONST_2,Opcodes.ICONST_3,Opcodes.DUP2_X1,Opcodes.IADD,Opcodes.IADD,Opcodes.IADD,Opcodes.IADD,Opcodes.IRETURN)},
            sample("dup2_x1_2"){insns(it,Opcodes.ICONST_1,Opcodes.LCONST_0,Opcodes.DUP2_X1,Opcodes.POP2,Opcodes.POP,Opcodes.POP2,Opcodes.ICONST_3,Opcodes.IRETURN)},
            sample("dup2_x2_1"){insns(it,Opcodes.ICONST_1,Opcodes.ICONST_2,Opcodes.ICONST_3,Opcodes.ICONST_4,Opcodes.DUP2_X2,Opcodes.IADD,Opcodes.IADD,Opcodes.IADD,Opcodes.IADD,Opcodes.IADD,Opcodes.IRETURN)},
            sample("dup2_x2_2"){insns(it,Opcodes.LCONST_0,Opcodes.ICONST_1,Opcodes.ICONST_2,Opcodes.DUP2_X2,Opcodes.POP2,Opcodes.POP2,Opcodes.POP2,Opcodes.ICONST_3,Opcodes.IRETURN)},
            sample("dup2_x2_3"){insns(it,Opcodes.ICONST_1,Opcodes.ICONST_2,Opcodes.LCONST_0,Opcodes.DUP2_X2,Opcodes.POP2,Opcodes.POP2,Opcodes.POP2,Opcodes.ICONST_3,Opcodes.IRETURN)},
            sample("dup2_x2_4"){insns(it,Opcodes.LCONST_0,Opcodes.DCONST_0,Opcodes.DUP2_X2,Opcodes.POP2,Opcodes.POP2,Opcodes.POP2,Opcodes.ICONST_3,Opcodes.IRETURN)},
            sample("swap"){insns(it,Opcodes.ICONST_1,Opcodes.ICONST_2,Opcodes.SWAP,Opcodes.ISUB,Opcodes.IRETURN)},
            sample("wide",locals=301){it.visitInsn(Opcodes.ICONST_3);it.visitVarInsn(Opcodes.ISTORE,300);it.visitIincInsn(300,1000);it.visitVarInsn(Opcodes.ILOAD,300);it.visitInsn(Opcodes.IRETURN)},
            sample("jsr",locals=1){val target=org.objectweb.asm.Label();it.visitJumpInsn(Opcodes.JSR,target);insns(it,Opcodes.ICONST_1,Opcodes.IRETURN);it.visitLabel(target);it.visitVarInsn(Opcodes.ASTORE,0);it.visitVarInsn(Opcodes.RET,0)},
        )
        val common=CommonSuperClassResolver { _,_->null }
        fun expectedJsrFailure(body:()->Unit){try{body()}catch(error:AssertionError){check(error.message?.contains("[JSR]")==true);}}
        for(bytes in samples)for(source in listOf("?","return @(?)","@(@(?) - @(?))","@(@(?) + @(?))","@(1+2)","return 3")){
            expectedJsrFailure{OfficialExpressionMatcher.match(bytes,"run","()I",listOf(source),OfficialExpressionMatchContextType.WRAP_OPERATION,commonSuperClass=common)}
            expectedJsrFailure{Capture.components{OfficialExpressionMatcher.match(bytes,"run","()I",listOf(source),OfficialExpressionMatchContextType.WRAP_OPERATION,commonSuperClass=common)}}
        }
        val handle=org.objectweb.asm.Handle(Opcodes.H_INVOKESTATIC,"java/lang/Integer","valueOf","(I)Ljava/lang/Integer;",false)
        for(value in listOf(org.objectweb.asm.Type.getMethodType("()Ljava/lang/String;"),handle,org.objectweb.asm.ConstantDynamic("value","Ljava/lang/String;",handle))){
            val bytes=sample("constantKind","()Ljava/lang/Object;"){it.visitLdcInsn(value);it.visitInsn(Opcodes.ARETURN)}
            for(source in listOf("?","@(? .class)"))OfficialExpressionMatcher.match(bytes,"run","()Ljava/lang/Object;",listOf(source),OfficialExpressionMatchContextType.WRAP_OPERATION,commonSuperClass=common)
        }
    }
    @Test fun receiverValidationEdges(){
        for(source in listOf("null","(null)","true","(true)","42","(42)","42.field","'text'","('text')","é","\$foo","𝕏","call('::')","call('/* name */')","foo/*name*/","foo//name","\u0085foo","\u001cfoo","super","new","new ?()","((value))","values[0]","(a+b)"))OfficialExpressionMemberCompletionProbes.build(source)
    }
    @Test fun isolatedUtf16Literals() {
        val common=CommonSuperClassResolver { _,_->null }
        for(payload in listOf(String(charArrayOf('\uD800')),String(charArrayOf('\uDC00')),String(charArrayOf('\uD800','\uDC00')),String(charArrayOf('a','\uD800','\u0000','\uDC00')),"�")) {
            val writer=ClassWriter(0);writer.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"oracle/Utf16Literal",null,"java/lang/Object",null)
            val method=writer.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,"run","()Ljava/lang/String;",null,null)
            method.visitCode();method.visitLdcInsn(payload);method.visitInsn(Opcodes.ARETURN);method.visitMaxs(1,0);method.visitEnd();writer.visitEnd()
            val bytes=writer.toByteArray();val source="'"+payload+"'"
            OfficialExpressionParser.parse(source)
            OfficialExpressionMatcher.match(bytes,"run","()Ljava/lang/String;",listOf(source),OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,commonSuperClass=common)
            OfficialExpressionMatcher.match(bytes,"run","()Ljava/lang/String;",listOf("'�'"),OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,commonSuperClass=common)
        }
    }
    @Test fun grammarEdges() {
        val sources = listOf(
            "a+b*c<<d&e^f|g", "a==b<c", "-a[0]", "~a+b", "(a)=b", "(a.b)=c", "(a[i])=b",
            "(Type).class", "(Type)::new", "Type[]::new", "?[][]::new", "new ?[a][b][]",
            "new ?[]{a,b,}", "new ?[]{a,b}", "new ?[]{}", "new ?[][]{{a}}", "(?)a.b(c)",
            "a instanceof ?[]", "return (a=b)", "throw new ?()", "@(@(a)+@(b))", "this::?",
            "::?", "?::new", "super.?()", "?.class", "?[][].class", "-9223372036854775808",
            "9223372036854775808", "0xffffffff", "0x80000000", "0x7fffffff", "1e3", ".5", "1.",
            "'🙂'", "'a\\'b'", "'a\\\\b'", "'a\\nb'", "a /*comment*/ + b", "a //comment\n + b",
            "é", "a\u00a0+b", "a\t+b", "\"text\"", "return", "a++", "a?b:c", "a=b=c",
        )
        for(source in sources) OfficialExpressionParser.parse(source)
    }
    @Test fun javacFlowCases() {
        val bytes = javaClass.getResourceAsStream("/oracle/SemanticSamples.class")!!.use { it.readBytes() }
        val methods = listOf(
            "arithmetic" to "(III)I", "longMath" to "(JJ)J", "floating" to "(DD)D",
            "intCompare" to "(II)Z", "longCompare" to "(JJ)Z", "floatCompare" to "(FF)Z",
            "doubleCompare" to "(DD)Z", "referenceCompare" to "(Ljava/lang/Object;Ljava/lang/Object;)Z",
            "increments" to "(I)I", "loop" to "(I)I", "table" to "(I)I", "lookup" to "(I)I",
            "exception" to "(Ljava/lang/String;)Ljava/lang/String;", "merge" to "(Z)Ljava/lang/Object;",
            "mergeArrays" to "(Z[Ljava/lang/String;[Ljava/lang/Integer;)[Ljava/lang/Object;",
            "literal" to "()[I", "multi" to "(II)[[Ljava/lang/String;", "partial" to "(I)[[Ljava/lang/String;",
            "array" to "([JIJ)J", "length" to "([Ljava/lang/Object;)I", "assignments" to "(I)I",
            "boxed" to "(Ljava/lang/Integer;)I", "concat" to "(ILjava/lang/String;)Ljava/lang/String;",
            "builder" to "(ILjava/lang/String;)Ljava/lang/String;",
            "bound" to "(Ljava/lang/String;)Ljava/util/function/Function;", "free" to "()Ljava/util/function/Function;",
            "constructor" to "()Ljava/util/function/Supplier;", "lambda" to "(I)Ljava/util/function/IntUnaryOperator;",
            "superCall" to "()Ljava/lang/String;", "classLiteral" to "()Ljava/lang/Object;", "unicode" to "()Ljava/lang/String;",
        )
        val expressions = listOf("?", "@(?+?)", "@(?-?)", "@(?*?)", "@(?/?)", "@(?%?)", "@(?<<?)", "@(?>>?)", "@(?>>>?)", "@(?&?)", "@(?|?)", "@(?^?)", "@(-?)", "@(~?)", "?==?", "?!=?", "?<?", "?<=?", "?>?", "?>=?", "return ?", "throw ?", "?=?", "?.?=?", "?[?]=?", "@(?[?])", "@(?.?)", "@(?.?())", "@(?())", "@(super.?())", "@(new ?())", "@(new ?[]{1,2,3})", "@(new ?[?][]) ", "@(new ?[?][?])", "@(?::?)", "@(::?)", "@(?::new)", "@(?[][].class)")
        val common = CommonSuperClassResolver { left,right -> if(left==right)left else "Ljava/lang/Object;" }
        for((name,descriptor) in methods) {
            for(source in expressions) {
                OfficialExpressionMatcher.match(bytes,name,descriptor,listOf(source),OfficialExpressionMatchContextType.WRAP_OPERATION,commonSuperClass=common)
                Capture.components{OfficialExpressionMatcher.match(bytes,name,descriptor,listOf(source),OfficialExpressionMatchContextType.WRAP_OPERATION,commonSuperClass=common)}
            }
        }
        for(context in OfficialExpressionMatchContextType.entries) {
            OfficialExpressionMatcher.match(bytes,"concat","(ILjava/lang/String;)Ljava/lang/String;",listOf("@(?+?)"),context,commonSuperClass=common)
            Capture.components{OfficialExpressionMatcher.match(bytes,"concat","(ILjava/lang/String;)Ljava/lang/String;",listOf("@(?+?)"),context,commonSuperClass=common)}
        }
    }
    @Test fun malformedFramesAndStacks() {
        fun bytes(stack:Int,locals:Int,code:(org.objectweb.asm.MethodVisitor)->Unit):ByteArray {
            val writer=ClassWriter(0);writer.visit(Opcodes.V1_8,Opcodes.ACC_PUBLIC,"oracle/Malformed",null,"java/lang/Object",null)
            val method=writer.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,"run","()I",null,null)
            method.visitCode();code(method);method.visitMaxs(stack,locals);method.visitEnd();writer.visitEnd();return writer.toByteArray()
        }
        val inputs=listOf(
            bytes(0,0){it.visitInsn(Opcodes.ICONST_1);it.visitInsn(Opcodes.IRETURN)},
            bytes(1,0){it.visitInsn(Opcodes.IADD);it.visitInsn(Opcodes.IRETURN)},
            bytes(1,0){it.visitVarInsn(Opcodes.ILOAD,3);it.visitInsn(Opcodes.IRETURN)},
            bytes(1,0){it.visitInsn(Opcodes.LCONST_0);it.visitInsn(Opcodes.IRETURN)},
            bytes(2,0){it.visitInsn(Opcodes.LCONST_0);it.visitInsn(Opcodes.POP);it.visitInsn(Opcodes.ICONST_1);it.visitInsn(Opcodes.IRETURN)},
            bytes(1,0){it.visitInsn(Opcodes.ICONST_1)},
        )
        for(input in inputs) OfficialExpressionMatcher.match(input,"run","()I",listOf("?"),OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,commonSuperClass=CommonSuperClassResolver { _,_->null })
        val framed=bytes(1,0){it.visitFrame(Opcodes.F_FULL,0,emptyArray<Any>(),0,emptyArray<Any>());it.visitInsn(Opcodes.ICONST_1);it.visitInsn(Opcodes.IRETURN)}
        check(framed[framed.size-9].toInt() and 255==255)
        val invalidTag=framed.copyOf().apply{this[size-9]=128.toByte()}
        val invalidDelta=framed.copyOf().apply{this[size-8]=127;this[size-7]=255.toByte()}
        val invalidCount=framed.copyOf().apply{this[size-11]=255.toByte();this[size-10]=255.toByte()}
        val objectFrame=bytes(1,1){it.visitFrame(Opcodes.F_FULL,1,arrayOf<Any>("java/lang/Object"),0,emptyArray<Any>());it.visitInsn(Opcodes.ICONST_1);it.visitInsn(Opcodes.IRETURN)}
        check(objectFrame[objectFrame.size-7].toInt() and 255==7)
        val invalidVerification=objectFrame.copyOf().apply{this[size-7]=9}
        val invalidClassIndex=objectFrame.copyOf().apply{this[size-6]=255.toByte();this[size-5]=255.toByte()}
        for(input in listOf(framed,invalidTag,invalidDelta,invalidCount,objectFrame,invalidVerification,invalidClassIndex))OfficialExpressionMatcher.match(input,"run","()I",listOf("?"),OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,commonSuperClass=CommonSuperClassResolver { _,_->null })
    }
}
