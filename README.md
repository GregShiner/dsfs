# Design
## Blocks
  - Currently BLOCK_SIZE is 4KiB but there should really be no reason this can't be variable
  - Block size minimum is 2KiB (maybe change in the future to 1KiB but this can cause problems with the initial padding)
  - Each block is indexed by a u32
  - This implies that a filesystem cannot contain more than 2^32 blocks
  - Blocks are 0-indexed

## Superblock
  - The super block contains metadata about the fs itself
  - It is always stored at block 0, the very first block
  - It is used to tell the driver important information about the fs
  - Since the driver has no way to know what the block size is before loading the fs, puttin the block size in the first block means that the driver does not need to know the block size ahead of time to find it.
  - Layout
    - 1024 Byte Padding (see [ext4 docs](https://ext4.wiki.kernel.org/index.php/Ext4_Disk_Layout#Layout))
    - u32: BLOCK_SIZE
    - u32: NUM_BLOCKS

## Groups
  - A group of blocks is a contiguous and aligned region of BLOCK_SIZE in bytes blocks
  - This is because the size of a group is determined by how many block types can fit in a single block table (see below)

## Block Table
  - The type of a block is determined by a corresponding u8 in the block table which is in the first block of a group
  - NOTE: This is not the case for the first group!
    - This first block in the fs is the superblock
    - For the first group, the block table is in block 1
    - All remaining groups have their block table in their first block
  - The block table occupies the entire first block in a group (hence why the number of blocks in a group is the number of u8s that can fit in a single block, which is also the number of bytes in a block)
  - Each byte in the block table determines the type of the block at the corresponding index
  - The first type in a block table is always BlockType::BlockTable because that is its block type (unless it is the first block table)
  - First block table
    - First 3 types are set
    1. Superblock: BlockType::Superblock
    2. Block table 0: BlockType::BlockTable
    3. Root directory inode: BlockType::Inode
  - If the size of an fs is not exactly a multiple of GROUP_SIZE, the block addresses that are invalid at the end of the last group are marked as BlockType::Unavailable

## Block Types
The types of a block are defined by an enum called BlockType. It has the following variants:
|Variant Name|Value|Description|
|------------|-----|-----------|
|Free|0x0|Unoccupied block ready for allocation|
|SuperBlock|0x1|Super Block|
|BlockTable|0x2|Block Table|
|Inode|0x3|Inode Entry|
|Data|0x4|Data Block|
|Error|0x5|Possibly dead block, do not use (not yet implemented)|
|IndirectionTable|0x6|Inode data indirection table (not yet implemented)|

## Inodes
  - Occupy 1 full block
  - Contain metadata about file (to be defined)
  - Contains block addresses for data as array of u32s which point to blocks (address table)
  - Can contain up to (BLOCK_SIZE - INODE_METADATA_SIZE) / sizeof(u32) addresses in a single block table
    - Indirection tables may be added in the future to allow additional data blocks. The following is a possible implementation:
    - When an address table is full and an additional data block is needed, allocate an additional block as a BlockType::IndirectionTable
    - Replace the last address table entry with the address of the new indirection table
    - Put the replaced address table entry into the first entry in the new indirection table
    - New data block allocations will go into the redirection table
    - This process will repeat with the second to last entry in the initial address table and continuing furthur up the table as more redirection tables are needed
    - This can also be done recursively with other redirection tables
    - Only when an address table is full, check each block against the block table to see if its a data block or redirection table (prob a more efficient way to do this)

## Inode Contents
  - u32: Parent directory inode address (root directory's parent is itself)
  - \[u8; 255\]: ASCII encoded filename. Cannot include NULL or '/'
  - u64: Filesize (holy shit this implies an absolutely bat shit insane max filesize)
  - u16: File Mode (lower 12 bits are POSIX permissions bits, upper 4 are POSIX file types)
  - Much more to be added to meet POSIX standards
  - \[u32; \<remaining space\>\]: Allocation table, contains list of block addresses

## Inode Types
  - Currently only Directories and Files. Sym Links will be added later
  - Directories
    - The address table contains the addresses of the inodes of its children
    - This includes a self-referential address with the name "." whose address is the block the current inode is in
    - It also includes a child with the name ".." which refers to the parent directory's inode
    - Hard links are created by creating a an entry in the allocation table with the targets inode
      - It is important to increment the reference count in the targets inode metadata
  - Files
    - The address table contains the addresses of all of the blocks that contain the data of the file
    - The order of the addresses determines the order that the data is in
  - Sym Links
    - Simply has its target inode address as the first and only entry of the address table

## Root
  - The root directory is always inode 0 and its inode is stored at block 2 (blocks are 0-indexed) after the superblock and free table 0

## FS Block Table
  This example assumes a 4KiB block size and a fs size of 256MiB (65536 Blocks)

  |Group 0   |             |              |                               |Group 1      |                               |
  |----------|-------------|--------------|-------------------------------|-------------|-------------------------------|
  |Block 0   |Block 1      |Block 2       |Blocks 3-4095                  |Block 4096   |Blocks 4097-8191               |
  |Superblock|Block Table 0|Root Dir Inode|Remaining data and inode blocks|Block Table 1|Remaining data and inode blocks|

## Inode Creation
  1. The allocator searches for an available block
  2. Mark block as BlockType::Inode in corresponding block table
  3. Create Inode initial metadata
  4. Create allocation table
     1. Files: Preallocate 1 data block, mark it as BlockType::Data and add it to allocation table
     2. Directories: Add "." and ".." entries to allocation table as described in `Inode Types -> Directories`

vim: ts=2:sw=2
