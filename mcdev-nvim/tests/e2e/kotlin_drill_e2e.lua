-- Frozen jdtls_bundle_e2e.lua:407-562; native routing, original Kotlin inputs/assertions.
return function(h)
  local helpers = h.helpers
  local convert = require("mcdev.convert")
  local workspace = h.workspace
  local workspace_root_uri = h.workspace_root_uri
  local client_id = h.client_id
  local find_line_and_character = h.find_line_and_character
  local log_step = h.log_step
  local protocol = require("mcdev.protocol")
  local kotlin_file = workspace .. "/src/main/kotlin/io/github/cotrin8672/cem/mixin/KotlinRequestMixin.kt"
  h.with_buffer(kotlin_file, "kotlin", nil, function(kotlin_buf)
    local kotlin_uri = vim.uri_from_bufnr(kotlin_buf)

    local function kotlin_text()
      return table.concat(vim.api.nvim_buf_get_lines(kotlin_buf, 0, -1, false), "\n")
    end

    local function set_kotlin(source)
      vim.api.nvim_buf_set_lines(kotlin_buf, 0, -1, false, vim.split(source, "\n", { plain = true }))
    end

    local function replace_once(source, old, new)
      local start = source:find(old, 1, true)
      helpers.assert_not_nil(start, "Kotlin source fragment missing: " .. old)
      return source:sub(1, start - 1) .. new .. source:sub(start + #old)
    end

    local function kotlin_request(command, marker, descriptor_mode)
      helpers.assert_eq(#vim.lsp.get_clients({ bufnr = kotlin_buf, name = "jdtls" }), 0,
        "JDTLS must not attach to Kotlin")
      helpers.assert_eq(protocol.active_ktlsp_client(kotlin_buf).id, client_id,
        "Kotlin must reuse the prepared workspace ktlsp client")
      local before = kotlin_text()
      local line, character = find_line_and_character(vim.api.nvim_buf_get_lines(kotlin_buf, 0, -1, false), marker)
      helpers.assert_not_nil(line, "Kotlin request marker missing: " .. marker)
      local position = { line, character + #marker }
      local payload = command == "mcdev.completion" and protocol.build_completion_payload(kotlin_buf, position)
        or { context = protocol.context(kotlin_buf, position) }
      helpers.assert_eq(payload.context.documentUri, kotlin_uri)
      helpers.assert_eq(payload.context.languageId, "kotlin")
      helpers.assert_eq(payload.context.workspaceRoot, workspace_root_uri)
      helpers.assert_eq(payload.context.bufferText or payload.context.bufferTextFallback, before)
      if descriptor_mode then
        payload.options.injectMethodDescriptor = descriptor_mode
      end
      local response, request_err = h.mcdev_command(command, payload, 60000)
      local result, err = convert.unwrap_envelope(response, request_err)
      helpers.assert_nil(err, command .. " Kotlin request failed")
      helpers.assert_not_nil(result, command .. " Kotlin result missing")
      helpers.assert_eq(kotlin_text(), before, command .. " must not mutate unsaved Kotlin")
      helpers.assert_eq(vim.uri_from_bufnr(kotlin_buf), kotlin_uri)
      return result
    end

    local function apply_kotlin_completion(partial, expected, descriptor_mode, expected_parse_source)
      local before = kotlin_text()
      local result = kotlin_request("mcdev.completion", partial, descriptor_mode)
      helpers.assert_eq(result.debug.parseSource, expected_parse_source)
      helpers.assert_eq(result.debug.usedCompilationUnit, false)
      local item = vim.tbl_filter(function(candidate)
        return candidate.edit and candidate.edit.newText == expected
      end, result.items or {})[1]
      helpers.assert_not_nil(item, "Kotlin completion missing: " .. expected)
      helpers.assert_eq(#(item.additionalEdits or {}), 0, "Kotlin value completion must not add Java imports")
      vim.lsp.util.apply_text_edits({ item.edit }, kotlin_buf, "utf-16")
      helpers.assert_eq(kotlin_text(), replace_once(before, partial, expected),
        "completion must edit only the original Kotlin value")
      helpers.assert_true(not kotlin_text():find(".class", 1, true), "Kotlin must not contain Java class literals")
      helpers.assert_true(not kotlin_text():match("import[^\n]*;"), "Kotlin imports must not have semicolons")
      helpers.assert_true(not kotlin_text():find("@At", 1, true), "Kotlin nested annotations must use At(...)")
      return item
    end

    local function assert_kotlin_diagnostics(marker)
      local diagnostics = kotlin_request("mcdev.diagnostics", marker).diagnostics or {}
      local errors = vim.tbl_filter(function(diagnostic) return diagnostic.severity == "error" end, diagnostics)
      helpers.assert_eq(#errors, 0, "valid Kotlin MC diagnostics: " .. vim.inspect(errors))
    end

    log_step("checking unsaved Kotlin workspace routing and exact completion edits")
    set_kotlin([[package io.github.cotrin8672.cem.mixin
import com.simibubi.create.content.kinetics.drill.DrillBlock as DrillTarget
import io.github.diskria.lapis.annotations.KMixin
import net.minecraft.core.BlockPos
import net.minecraft.world.InteractionHand
import net.minecraft.world.ItemInteractionResult
import net.minecraft.world.entity.player.Player
import net.minecraft.world.item.ItemStack
import net.minecraft.world.level.Level
import net.minecraft.world.level.block.state.BlockState
import net.minecraft.world.phys.BlockHitResult
import org.spongepowered.asm.mixin.injection.Inject
import org.spongepowered.asm.mixin.injection.At
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable
@KMixin(target = DrillTarget::class)
class KotlinDrillMixin {
    @Inject(method = ["useIt"], at = [At("HE")], cancellable = true)
    fun DrillTarget.onUse(stack: ItemStack, state: BlockState, level: Level, pos: BlockPos, player: Player, hand: InteractionHand, hitResult: BlockHitResult, cir: CallbackInfoReturnable<ItemInteractionResult>) {}
}]])
    local drill_descriptor = "(Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/level/block/state/BlockState;"
      .. "Lnet/minecraft/world/level/Level;Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/entity/player/Player;"
      .. "Lnet/minecraft/world/InteractionHand;Lnet/minecraft/world/phys/BlockHitResult;)Lnet/minecraft/world/ItemInteractionResult;"
    local drill_selector = "useItemOn" .. drill_descriptor
    local method_item = apply_kotlin_completion("useIt", drill_selector, "always", "HOST_NORMALIZED")
    helpers.assert_eq(method_item.metadata.owner, "com/simibubi/create/content/kinetics/drill/DrillBlock")
    helpers.assert_eq(method_item.metadata.descriptor, drill_descriptor)
    apply_kotlin_completion("HE", "HEAD", nil, "HAND_WRITTEN_FALLBACK")
    assert_kotlin_diagnostics("useItemOn")
    local hover = kotlin_request("mcdev.hover", "useItemOn")
    helpers.assert_true(table.concat(hover.contents or {}, "\n"):find("useItemOn", 1, true) ~= nil,
      "Kotlin hover must resolve the unsaved method selector")
    local definitions = kotlin_request("mcdev.definition", "useItemOn").locations or {}
    local method_location = vim.tbl_filter(function(location)
      return location.metadata and location.metadata.name == "useItemOn"
    end, definitions)[1]
    helpers.assert_not_nil(method_location, "Kotlin method definition missing")
    helpers.assert_eq(method_location.metadata.descriptor, drill_descriptor)
    helpers.assert_eq(method_location.resolution, "source")
    helpers.assert_true(method_location.documentUri:find("DrillBlock.java", 1, true) ~= nil,
      "Kotlin definition must reuse the real Java dependency source")
    set_kotlin(replace_once(kotlin_text(), drill_selector, "doesNotExist"))
    local bad_diagnostics = kotlin_request("mcdev.diagnostics", "doesNotExist").diagnostics or {}
    helpers.assert_true(vim.tbl_filter(function(diagnostic)
      return diagnostic.code == "MIXIN_UNRESOLVED_INJECT_METHOD"
    end, bad_diagnostics)[1] ~= nil, "bad unsaved Kotlin selector must be diagnosed")

    set_kotlin([[package io.github.cotrin8672.cem.mixin
import net.createmod.catnip.placement.PlacementOffset as PlacementTarget
import io.github.diskria.lapis.annotations.KMixin
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation
import com.llamalad7.mixinextras.injector.wrapoperation.Operation
import java.lang.Void
import net.minecraft.core.BlockPos
import net.minecraft.world.entity.LivingEntity
import net.minecraft.world.item.ItemStack
import net.minecraft.world.level.Level
import net.minecraft.world.level.block.Block
import net.minecraft.world.level.block.state.BlockState
import org.spongepowered.asm.mixin.injection.At
@KMixin(target = PlacementTarget::class)
class KotlinPlacementMixin {
    @WrapOperation(method = ["placeInW"], at = At(value = "INVOKE", target = "Lnet/minecraft/world/level/block/Block;setPl", remap = true), remap = false)
    fun PlacementTarget.onPlaced(instance: Block, level: Level, pos: BlockPos, state: BlockState, placer: LivingEntity, stack: ItemStack, original: Operation<Void>) {}
}]])
    apply_kotlin_completion("placeInW", "placeInWorld", "never", "HOST_NORMALIZED")
    local block_target = "Lnet/minecraft/world/level/block/Block;setPlacedBy(Lnet/minecraft/world/level/Level;"
      .. "Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;"
      .. "Lnet/minecraft/world/entity/LivingEntity;Lnet/minecraft/world/item/ItemStack;)V"
    local target_item = apply_kotlin_completion("Lnet/minecraft/world/level/block/Block;setPl", block_target, nil, "HOST_NORMALIZED")
    helpers.assert_eq(target_item.metadata.name, "setPlacedBy")
    assert_kotlin_diagnostics("placeInWorld")
    helpers.assert_eq(#vim.lsp.get_clients({ bufnr = kotlin_buf, name = "jdtls" }), 0)
    helpers.assert_eq(vim.fn.filereadable(kotlin_file), 0, "Kotlin request source must remain unsaved")
    io.stdout:write("acceptance: Kotlin workspace reuse without JDT attach; method/At/target exact edits, diagnostics, hover/definition passed\n")
  end)
end
