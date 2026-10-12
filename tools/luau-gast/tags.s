# Ausnahme-"Tags" für WebAssembly-Exceptions (C++-Ausnahmen und setjmp/longjmp).
# Normalerweise liefert die Laufzeitbibliothek sie; hier definieren wir sie selbst.
	.globl	__cpp_exception
	.tagtype	__cpp_exception i32
__cpp_exception:
	.globl	__c_longjmp
	.tagtype	__c_longjmp i32
__c_longjmp:
