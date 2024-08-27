* Blocks
  - Currently BLOCK_SIZE is 4KiB but there should really be no reason this can't be variable
  - Each block is indexed by a u32
  - This implies that a filesystem cannot contain more than 2^32 blocks
  - Blocks are 0-indexed

* Superblock
  - The super block contains metadata about the fs itself
  - It is always stored at block 1 (immediately after the group 0 free table)
  - Layout
    - u32: BLOCK_SIZE
    - u32: NUM_BLOCKS
    - u32: BLOCKS_IN_GROUP

* Groups
  - A group of blocks is a contiguous and aligned region of 8 * BLOCK_SIZE blocks (the number of bits in a single block)

* Free Table
  - Free and used blocks are denoted by a free table in the first block of a group
  - The free table occupies the entire first block in a group (hence why the number of blocks in a group is the number of bits in a block)
  - In the case of the first group, the first block is the superblock, and the second block is the free table
  - Each bit in the free table is 1 if the block is occupied, and 0 if free
  - The first bit in a free table is always 1 because it itself is in an occupied block (unless it is the first free table)
  - First free table
    - First 3 bits are set
    1. Superblock
    2. Free table 0
    3. Root dir inode
  - If the size of an fs is not exactly a multiple of GROUP_SIZE, the block addresses that are invalid at the end of the last group are marked as occupied

* Inodes
  - Occupy 1 full block
  - Contain metadata about file
  - Contains block addresses for data as array of u32s which point to blocks
  - Can contain up to (BLOCK_SIZE - INODE_METADATA_SIZE) / 4 addresses in a single block table
    - Consider looking into indirection tables in the future

* Inode Types
  - Currently only Directories and Files. Sym Links will be added later
  - Directories
    - The address table contains the addresses of the inodes of its children
    - This includes a self-referential address with the name "." whose address is the block the current inode is in
    - It also includes a child with the name ".." which refers to the parent directory's inode
  - Files
    - The address table contains the addresses of all of the blocks that contain the data of the file
    - The order of the addresses determines the order that the data is in

* Root
  - The root directory is always inode 0 and its inode is stored at block 2 (blocks are 0-indexed) after free table 0 and the superblock

* FS Block Table
  This example assumes a 4KiB block size and a fs size of 256MiB (65536 Blocks)

  |Group 0   |            |              |                               |Group 1     |                               |
  |----------|------------|--------------|-------------------------------|------------|-------------------------------|
  |Block 0   |Block 1     |Block 2       |Blocks 3-32767                 |Block 32768 |Blocks 32768-65535             |
  |Superblock|Free Table 0|Root Dir Inode|Remaining data and inode blocks|Free Table 1|Remaining data and inode blocks|

* Inode Creation
  1. The allocator searches for an available block
  2. Mark block as occupied
  3. Create Inode initial metadata
  4. Create allocation table
     1. Files: Preallocate 1 data block, mark it as occupied and add it to allocation table
     2. Directories: Add "." and ".." entries to allocation table as described in `Inode Types -> Directories`

vim: ts=2:sw=2
