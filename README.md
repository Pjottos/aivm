# CPU AI
Artificial intelligence that embraces the hardware it runs on.

Instead of relying on huge matrix multiplications and non-linear activation functions,
`cpu_ai` uses native machine code to directly drive it's decision making. This removes an
expensive layer of abstraction from typical artificial intelligence agents.

## Agent structure
An agent has the following components:
- __Thread__
   The CPU thread that runs the machine code.
- __Memory__
   The short and long term memory of the agent. Also used to pass sensory input and read
   output.
- __Stack__
   A space for local variables in functions (see below). It consists of 256 8 byte values.
- __Code__
   A collection of *functions* with one entry point. Functions have no arguments and do not
   return values, it is expected that information is shared through the memory. The entry
   point is executed once at each *step*, which is similar to a forward pass.

## Code generation
Each program has the following parameters:
- __Memory__
   The initial values for the agent's memory, in 8 byte chunks.
- __Code string__
   A string of 64 bit values that is used to generate the actual code.

A mix of integer, bitwise, call, load/store and conditional branch instructions is used.
Infinite loops are prevented by only allowing a branch to jump to a later part in the same
function, and by turning recursive function calls into a no-op.

For details of instruction encoding, check `compile.rs`.
