"""Author the narrowly reviewed bug adaptations from original inputs, never Rust outputs.

Run separately from capture. Every selector, replacement and regression below is explicit.
Original calls/assertions remain unmodified. Parent approved these range/edit/newline fixes.
"""
from pathlib import Path
import json
HERE=Path(__file__).resolve().parent
rows=[json.loads(line) for line in (HERE/'legacy_calls.jsonl').read_text().splitlines()]
result=[]

def row(cls,method,api):
    matches=[r for r in rows if r['legacyCase'].endswith('/'+cls+'.kt::'+method) and r['api']==api]
    assert len(matches)==1,(cls,method,api,len(matches))
    return matches[0]

def add(cls,method,api,changes,reason,regression):
    r=row(cls,method,api)
    entries=[]
    for pointer,old,new in changes:
        v=r['output']
        if pointer:
            for key in pointer.strip('/').split('/'):v=v[int(key)] if isinstance(v,list) else v[key]
        assert v==old,(cls,method,pointer,v,old)
        entries.append({'field':pointer,'old':old,'new':new})
    result.append({'legacyCase':r['legacyCase'],'api':api,'args':r['args'],'changes':entries,'reason':reason,'regression':regression})

for cls,method,old,new in [
 ('AtContextExtractorTest','detectsOwnerSlotAfterModifierOnTrailingCursor',0,7),
 ('AtContextExtractorTest','detectsMemberSlotAfterOwnerOnTrailingCursor',0,44),
 ('AtContextExtractorTest','ignoresCommentSuffix',0,53),
 ('AccessTransformerCompletionServiceTest','returnsEmptyWhenOwnerMissingForMemberCompletion',0,7),
 ('AccessTransformerCompletionServiceTest','separatesDisplayLabelFromInsertText',0,44),
]:
    for api in ['AtContextExtractor.extractAtOffset','AtContextExtractor.extract']:
        r=row(cls,method,api);source=r['args']['source']
        add(cls,method,api,[('/valueStartOffset',old,new)],'Missing token replacement starts at the caret, preserving preceding modifier/owner/comment.',
            {'kind':'contextRange','source':source,'start':new,'end':new,'selectedText':''})

for cls,method in [('AtContextExtractorTest','detectsMemberDescriptorSlot'),('AccessTransformerCompletionServiceTest','returnsEmptyForDescriptorSlot')]:
    for api in ['AtContextExtractor.extractAtOffset','AtContextExtractor.extract']:
        r=row(cls,method,api)
        add(cls,method,api,[('/partialValue','s','(')],'Descriptor partial uses the descriptor token offset instead of indexing from the member name.',
            {'kind':'contextRange','source':r['args']['source'],'start':53,'end':54,'selectedText':'('})

for cls,method in [
 ('AccessWidenerDiagnosticsServiceTest','detectsDescriptorMismatch'),
 ('AccessWidenerCodeActionServiceTest','offersFixDescriptorForMismatch'),
 ('AccessWidenerCodeActionServiceTest','applyDescriptorFixReplacesWrongMethodDescriptor'),
]:
    r=row(cls,method,'AccessWidenerDiagnosticsService.analyze')
    add(cls,method,'AccessWidenerDiagnosticsService.analyze',[('/0/range/end/character',67,68)],
        'Descriptor highlight includes its final character; source token is the full ()V.',
        {'kind':'diagnosticRange','source':r['args']['request']['source'],'selectedText':'()V'})

for method,replacement in [
 ('appliesAddDescriptorFix','setScreen(Lnet/minecraft/client/gui/screen/Screen;)V'),
 ('appliesRemapFixToSrgNames','m_91152_(Lnet/minecraft/client/gui/screen/Screen;)V'),
 ('remapActionUsesExactDescriptorWhenDescriptorIsPresent','m_string_(Ljava/lang/String;)V'),
 ('remapActionUsesSingleCandidateOnlyWhenDescriptorIsOmitted','m_tick_()V'),
]:
    api='AccessTransformerCodeActionService.'+('applyMethodDescriptorFix' if method=='appliesAddDescriptorFix' else 'applyRemapFix')
    r=row('AccessTransformerCodeActionServiceTest',method,api);source=r['args']['currentContent']
    start=source.rindex(' ')+1
    edits=[{'startOffset':start,'endOffset':len(source),'newText':replacement}]
    document=source[:start]+replacement
    add('AccessTransformerCodeActionServiceTest',method,api,[('/edits',r['output']['edits'],edits)],
        'Replace only the member token; preserve all other text. Original and revised edits produce exactly the same document.',
        {'kind':'equivalentEdits','source':source,'document':document})

for cls,method,editor,action,start,end,document in [
 ('AccessWidenerCodeActionServiceTest','applyRemoveDuplicateFixRemovesLine','AccessWidenerEditor.removeLine','AccessWidenerCodeActionService.applyRemoveDuplicateFix',77,130,'accessWidener v2 named\naccessible class net/minecraft/client/MinecraftClient\n'),
 ('AccessTransformerCodeActionServiceTest','appliesRemoveDuplicateFix','AccessTransformerEditor.removeLine','AccessTransformerCodeActionService.applyRemoveDuplicateFix',44,87,'public net.minecraft.client.MinecraftClient\n'),
]:
    r=row(cls,method,editor);field='/content' if 'Transformer' in editor else ''
    old=r['output']['content'] if field else r['output']
    add(cls,method,editor,[(field,old,document)],'Deleting the final duplicate preserves the preceding line terminator.',
        {'kind':'document','document':document})
    r=row(cls,method,action);source=r['args']['currentContent']
    add(cls,method,action,[('/edits',r['output']['edits'],[{'startOffset':start,'endOffset':end,'newText':''}])],
        'Delete the duplicate range while preserving the preceding line terminator. Bug-corrected adaptation: count entries in the applied document instead of replacement newText.',
        {'kind':'changedEdits','source':source,'oldDocument':document.rstrip('\n'),'document':document,'assertion':'applied document contains exactly one accessible class/public owner entry'})

cls='AccessWidenerCodeActionServiceTest';method='applyGenerateEntryFixInsertsLine';api='AccessWidenerCodeActionService.applyGenerateEntryFix'
r=row(cls,method,api);source=r['args']['currentContent'];entry='accessible class net/minecraft/client/MinecraftClient\n'
add(cls,method,api,[('/edits',r['output']['edits'],[{'startOffset':0,'endOffset':0,'newText':entry}])],
    'Insertion preserves existing terminators instead of reconstructing the document and adding a blank line.',
    {'kind':'changedEdits','source':source,'oldDocument':entry+source+'\n','document':entry+source})

cls='AccessWidenerEditorTest';method='insertEntryAppendsFormattedLine';api='AccessWidenerEditor.insertEntry'
r=row(cls,method,api)
add(cls,method,api,[('',r['output'],'accessWidener v2 named\n'+entry)],
    'Appending to an already terminated document adds the entry without inventing a blank line.',
    {'kind':'document','document':'accessWidener v2 named\n'+entry})

assert len(result)==27,len(result)
(HERE/'intentional_deltas.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print('explicit approved adaptations='+str(len(result)))
