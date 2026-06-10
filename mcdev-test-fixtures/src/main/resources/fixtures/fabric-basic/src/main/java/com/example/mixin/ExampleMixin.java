package com.example.mixin;

import com.example.target.SimpleTarget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(SimpleTarget.class)
public abstract class ExampleMixin {
    @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))
    private void mcdev$onDraw(String text, float x, float y, CallbackInfo ci) {
    }
}
