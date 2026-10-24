package com.example.mixin;

import com.example.target.SimpleTarget;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

@Mixin(SimpleTarget.class)
public abstract class MixinExtrasExample {
    @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
    private float mcdev$modifyX(float original) {
        return original;
    }

    @ModifyReturnValue(method = "draw(Ljava/lang/String;FF)V", at = @At("RETURN"))
    private void mcdev$modifyReturn(void original) {
    }

    @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
    private int mcdev$wrapLength(Operation<Integer> original, String instance) {
        return original.call(instance);
    }
}
