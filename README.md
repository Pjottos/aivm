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
   A space for local variables in functions (see below).
- __Code__
   A collection of *functions* with one entry point. Functions have no arguments and do not
   return values, it is expected that information is shared through the memory. The entry
   point is executed once at each *step*, which is similar to a forward pass.

## Code generation
Each program has the following parameters:
- __Memory size__
   Amount of 8 byte chunks that make up the agent's memory.
- __Code string__
   A string of 32 bit floating point numbers that is used to generate the actual code.

A mix of integer, bitwise, floating point, call, load/store and if/else instructions
is used. Recursive function calls are turned into no-ops and by extension (infinite) loops
are impossible.

Instructions are encoded with a variable amount of numbers and therefore the amount of
functions is only known after parsing the whole code string. For details of instruction
encoding, check the source code.
