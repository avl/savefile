# WARNING

This software is under heavy development right now and NOT ready for use.


# savefile 

Savefile is a library to effortlessly serialize rust structs and enums, in
an efficient binary format, to anything implementing the Write trait, and 
then deserializing the same from anything implementing the Read trait. This 
means that savefile can be used to easily save in memory data structures to 
disk for persistent storage.

You may ask what savefile brings to the table that serde doesn't already do
better. The answer is: Not much! However, Savefile is much smaller and less 
complex, which could sometimes be an advantage in itself. Savefile also
has features to easily maintain backward compatibility to old versions
of the software.


Features savefile has:

 * Fast binary serialization and deserialization
 * Support for old versions of the save format
 * Completely automatic implementation using "custom derive". You do not have to
 figure out how your data is to be saved.
 
Features savefile does not have, and will not have:

 * Support for external protocols/data formats. There'll never be json, yaml,
 xml or any other backends. Savefile uses the savefile format, period.
 * Support for serializing graphs. Savefile can serialize your data if it has a
 tree structure in RAM, _without_ loops.
 * Support for serializing boxed traits. You can hack this in by manually
 implementing the Serialize and Deserialize traits.
 
 

