import io.github.diskria.lapis.annotations.KMixin
import io.github.diskria.lapis.annotations.KShadow as Shadowed
import io.github.diskria.lapis.annotations.MappingName as Mapped
import javax.lang.model.element.Modifier.PRIVATE as Hidden
import javax.lang.model.element.Modifier.STATIC as ClassMember
import game.Target
@KMixin(Target::class)
abstract class Patch {
    @Shadowed(Hidden)
    @Mapped(name = "count")
    abstract var observed: Int
    @Mapped("secret")
    @Shadowed(Hidden)
    abstract fun hidden(argument: Int): Int
    @Shadowed(ClassMember)
    @Mapped("manualTotal")
    abstract val manualView: Long
    companion object {
        @Shadowed
        @Mapped("total")
        val totalView: Long = 0
    }
}
