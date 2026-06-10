package com.example.mixin;

import com.example.target.SimpleTarget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(value = SimpleTarget.class)
public abstract class BrokenMixin {
    @Inject(method = "missingMethod(Ljava/lang/String;)V", at = @At("HEAD"))
    private void mcdev$onMissing(String text, CallbackInfo ci) {
    }

    @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Lcom/example/missing/Missing;run()V"))
    private void mcdev$badAtTarget(String text, float x, float y, CallbackInfo ci) {
    }
}
