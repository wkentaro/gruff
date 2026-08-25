# Require private inputs

RH002 flags each defaulted fixed caller-supplied input to a private module-level function or method. Requiring callers to supply every value makes private behavior explicit without caller analysis; the specification sweep found about 57 affected definitions across the reference repositories. Implicit method receivers and variadic parameters are excluded. The rule remains opt-in while repository trials establish that required private inputs reliably reduce review work.
