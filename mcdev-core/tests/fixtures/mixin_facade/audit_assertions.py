"""Record every frozen original body/assertion and its native witnesses.

This is an audit ledger, not an assertion interpreter: final service calls must
be captured whole or have an explicit native tail test.
"""
from pathlib import Path
import collections, hashlib, importlib.util, json, re
HERE=Path(__file__).resolve().parent; ROOT=HERE.parents[3]
BASE=ROOT/'build/migration-baseline/current-20261023'
spec=importlib.util.spec_from_file_location('source_capture',HERE.parent/'mixin_source/generate_capture.py')
helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)

def skip_literal(text,i):
    end=helper.skip_literal(text,i)
    # Kotlin raw strings may end with four quotes: the leading quote belongs
    # to the value and the final three terminate it.
    if end is not None and text.startswith('"""',i):
        while end<len(text) and text[end]=='"':end+=1
    return end

def body(text,name):
    match=re.search(r'\bfun\s+'+re.escape(name)+r'\s*\(',text)
    assert match,name
    start=text.index('{',helper.closing(text,text.index('(',match.start()))+1)
    i=start;depth=0
    while i<len(text):
        end=skip_literal(text,i)
        if end is not None:i=end;continue
        if text[i]=='{':depth+=1
        elif text[i]=='}':
            depth-=1
            if not depth:return text[start:i+1]
        i+=1
    raise ValueError(name)

def code_only(text):
    result=[];i=0
    while i<len(text):
        end=skip_literal(text,i)
        if end is not None:result.append(' '*(end-i));i=end
        else:result.append(text[i]);i+=1
    return ''.join(result)

def main():
    coverage=json.loads((HERE/'coverage.json').read_text())
    audit=[];counts=collections.Counter()
    for row in coverage:
        text=(BASE/row['legacyTestPath']).read_text(encoding='utf8')
        original=body(text,row['legacyCase']);code=code_only(original)
        assertions=[]
        for match in re.finditer(r'\b(assert\w+)(?:<[^\n]+?>)?\s*\(',code):
            start=code.index('(',match.start());end=helper.closing(original,start)
            assertions.append(original[match.start():end+1])
        calls=re.findall(r'\b([\w.]+)\s*\(',code)
        counts.update(calls)
        audit.append(dict(originalId=row['originalId'],bodySha256=hashlib.sha256(original.encode()).hexdigest(),
            assertions=assertions,calls=calls,wholeCapturedApis=list(row['apiCounts']),nativeWitness=row['rustTest'],
            additionalWitnesses=[row[k]for k in ['tailAssertion','statefulAssertion','authorityAssertion','directAssertion']if k in row],
            assertionPreservation=row.get('assertionPreservation','Whole public API return payloads, semantic variants and strict callback reads; immutable original assertions passed in JVM oracle.')))
    assert len(audit)==189
    # Check all result-producing product calls appearing directly in an
    # original body, not just cases with no facade rows. This catches action
    # and validation tails following an already captured diagnose/complete.
    product_methods={
        'complete','completeWithDebug','diagnose','codeActions','analyzeDiagnostics',
        'tryCompleteBufferOnly','semanticModel','definitions','definitionsAtOffset',
        'applyMixinConfigFix','applyMethodDescriptorFix','fixesForDiagnostics',
        'completeFields','selectedMixinConfig','collectSourceEntries',
    }
    native_methods={
        'generateAccessorFixContainsFieldName':{'fixesForDiagnostics'},
        'producesGenerateInvokerFix':{'fixesForDiagnostics'},
        'producesGenerateAccessorFix':{'fixesForDiagnostics'},
        'routesShadowMemberCompletion':{'completeFields'},
        'kotlinSourcesUseSemicolonlessPackagesForConfigAndRegisteredShareLookup':{'selectedMixinConfig','collectSourceEntries'},
    }
    for row,item in zip(coverage,audit):
        captured={api.rsplit('.',1)[-1]for api in item['wholeCapturedApis']}
        explicit=native_methods.get(row['legacyCase'],set())
        required={call.rsplit('.',1)[-1]for call in item['calls']}&product_methods
        assert required<=captured|explicit,(row['originalId'],required-captured-explicit)
        item['directProductMethods']=sorted(required)
        item['nativeOnlyProductMethods']=sorted(explicit)
    (HERE/'assertion-audit.json').write_text(json.dumps(audit,indent=2)+'\n',encoding='utf8')
    summary=dict(cases=len(audit),explicitBodyAssertionExpressions=sum(len(x['assertions'])for x in audit),
        originalAssertionFunctions=dict(collections.Counter(assertion.split('(')[0].split('<')[0]for item in audit for assertion in item['assertions'])),
        resultProducingProductMethodsChecked=sorted(product_methods),missingNativeProductMethodWitnesses=[],
        missingOriginalAssertionsFoundByReview=['generateAccessorFixContainsFieldName','producesGenerateInvokerFix','producesGenerateAccessorFix','routesShadowMemberCompletion','kotlinSourcesUseSemicolonlessPackagesForConfigAndRegisteredShareLookup'],
        additionalComputedEditTails=['codeActionFixesWrapOperationBadReturnThroughFacade','facadeNormalizesDeclarationSnippetIndentationAtBothInsertionBoundaries'],
        originalCaseCountUnchanged=189,
        methodology='Every frozen original body and explicit assertion expression is retained with a body hash; service/caller calls are audited against whole captured APIs or exact independent native functions. Callback/helper assertions are preserved by strict argument/order/count traces and native same-instance state/authority/live-file tests. Counts are static body expressions, not loop-expanded executions; original JVM assertions still execute unchanged.')
    (HERE/'assertion-audit-summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf8')
    print(json.dumps(summary))
if __name__=='__main__':main()
