We want to support multiple layers of collaboration with different requirements. Version Control should fluently support both and be coupled with the underlying graph structure of the program.

# Realtime
Collaboration with small team of people (trust based system). Conflict handling is done automatically with ongoing editing from team members.
History only session based with potential snapshots of changes.

# Asynchronous
Following pull-request model to scale to larger number of people. Merges require assisting from users to be conflict free.

### Requirements
- sync up sub layers (expanded codes)
- support multi-target layers of the machine code
- share codegen work

### References
- CRDTs: Issue sync up graph layers (rewrites) beside the main graph layer. Might need to run through a central server anyway?

