package io.github.mcdev.core.mapping

import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class MappingParserTest {
    @Test
    fun parsesTinyV2ClassesAndMembers() {
        val result = assertIs<MappingParseResult.Success>(
            TinyV2Parser.parse(
                """
                tiny	2	0	named	intermediary
                c	net/minecraft/client/MinecraftClient	net/minecraft/class_310
                	m	()V	tick	method_1574
                	f	Lnet/minecraft/client/gui/screen/Screen;	currentScreen	field_1755
                """.trimIndent(),
            ),
        )

        assertEquals(
            "net/minecraft/class_310",
            result.mappings.className("net/minecraft/client/MinecraftClient", MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY),
        )
        assertEquals(
            "method_1574",
            result.mappings.methodName("net/minecraft/client/MinecraftClient", "tick", "()V", MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY),
        )
    }

    @Test
    fun parsesSrgMappings() {
        val result = assertIs<MappingParseResult.Success>(
            SrgParser.parse(
                """
                CL: net/minecraft/client/Minecraft net/minecraft/client/Minecraft
                MD: net/minecraft/client/Minecraft/func_91152_a (Ljava/lang/String;)V net/minecraft/client/Minecraft/m_91152_ (Ljava/lang/String;)V
                FD: net/minecraft/client/Minecraft/field_91074_ net/minecraft/client/Minecraft/f_91074_
                """.trimIndent(),
            ),
        )

        assertEquals(
            "m_91152_",
            result.mappings.methodName("net/minecraft/client/Minecraft", "func_91152_a", "(Ljava/lang/String;)V", MappingNamespace.OFFICIAL, MappingNamespace.SRG),
        )
    }

    @Test
    fun remapsClassesMembersAndDescriptors() {
        val mappings = assertIs<MappingParseResult.Success>(
            TinyV2Parser.parse(
                """
                tiny	2	0	named	intermediary
                c	net/minecraft/client/MinecraftClient	net/minecraft/class_310
                	m	(Lnet/minecraft/client/gui/screen/Screen;)V	setScreen	method_1507
                	f	Lnet/minecraft/client/gui/screen/Screen;	currentScreen	field_1755
                c	net/minecraft/client/gui/screen/Screen	net/minecraft/class_437
                """.trimIndent(),
            ),
        ).mappings.asResolver()

        val method = assertIs<MappingLookupResult.Found<MethodRef>>(
            mappings.remapMethod(
                MethodRef(
                    owner = "net/minecraft/client/MinecraftClient",
                    name = "setScreen",
                    descriptor = "(Lnet/minecraft/client/gui/screen/Screen;)V",
                    namespace = MappingNamespace.NAMED,
                ),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("net/minecraft/class_310", method.value.owner)
        assertEquals("method_1507", method.value.name)
        assertEquals("(Lnet/minecraft/class_437;)V", method.value.descriptor)

        val field = assertIs<MappingLookupResult.Found<FieldRef>>(
            mappings.remapField(
                FieldRef(
                    owner = "net/minecraft/client/MinecraftClient",
                    name = "currentScreen",
                    descriptor = "Lnet/minecraft/client/gui/screen/Screen;",
                    namespace = MappingNamespace.NAMED,
                ),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("net/minecraft/class_310", field.value.owner)
        assertEquals("field_1755", field.value.name)
        assertEquals("Lnet/minecraft/class_437;", field.value.descriptor)
    }

    @Test
    fun reportsMissingMemberMappingSeparatelyFromMissingClassMapping() {
        val resolver = assertIs<MappingParseResult.Success>(
            TinyV2Parser.parse(
                """
                tiny	2	0	named	intermediary
                c	net/minecraft/client/MinecraftClient	net/minecraft/class_310
                """.trimIndent(),
            ),
        ).mappings.asResolver()

        val missingMember = assertIs<MappingLookupResult.Missing>(
            resolver.remapMethod(
                MethodRef(
                    owner = "net/minecraft/client/MinecraftClient",
                    name = "missing",
                    descriptor = "()V",
                    namespace = MappingNamespace.NAMED,
                ),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals(MappingSubject.METHOD, missingMember.subject)

        val missingClass = assertIs<MappingLookupResult.Missing>(
            resolver.remapClass(
                ClassRef("net/minecraft/client/Unknown", MappingNamespace.NAMED),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals(MappingSubject.CLASS, missingClass.subject)
    }
}
