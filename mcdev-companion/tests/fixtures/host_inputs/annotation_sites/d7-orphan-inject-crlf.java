package io.github.cotrin8672.cem.mixin;
import net.minecraft.world.item.Item;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.Inject;
@Mixin(Item.class)
public class ItemMixin {
    @Inject(method = "is")
}
