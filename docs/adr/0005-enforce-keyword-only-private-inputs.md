# Enforce keyword-only private inputs

RH001 flags each positional fixed caller-supplied input to a private module-level function or method. Requiring keyword-only inputs makes names visible at every call site without caller analysis. Implicit method receivers and variadic parameters are excluded. The rule remains opt-in while repository trials establish that named private inputs reliably reduce review work.
