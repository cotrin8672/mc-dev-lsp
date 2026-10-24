@Mixin(LocalCaptureSamples.class)
abstract class ExampleMixin {
    @WrapOperation(
        method = "instanceWithArgs(Ljava/lang/String;I)I",
        at = @At(
            value = "MIXINEXTRAS:EXPRESSION",
            args = {
                @Definition(id = "loc", local = { @Local( }),
                @Expression("0")
            }
        )
    )
    private int mcdevHandler(String message, Operation<Integer> original) {
        return original.call(message);
    }
}