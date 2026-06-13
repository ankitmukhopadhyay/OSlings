/*
 * Linker script for the rv6 kernel.
 *
 * QEMU's `virt` machine, started with `-bios none`, places the kernel ELF in
 * RAM and jumps to its ENTRY symbol at the RAM base, 0x8000_0000. Everything
 * below describes how our sections are laid out starting there.
 *
 * You read and annotate this file in exercise 01.
 */

OUTPUT_ARCH( "riscv" )
ENTRY( _entry )            /* the first instruction QEMU runs */

SECTIONS
{
  . = 0x80000000;         /* RAM base on the `virt` machine */

  .text : {
    *(.entry)             /* _entry must come first, at 0x80000000 */
    *(.text .text.*)
    . = ALIGN(0x1000);
    PROVIDE(etext = .);
  }

  .rodata : {
    . = ALIGN(16);
    *(.srodata .srodata.*)
    *(.rodata .rodata.*)
  }

  .data : {
    . = ALIGN(16);
    *(.sdata .sdata.*)
    *(.data .data.*)
  }

  .bss : {
    . = ALIGN(16);
    *(.sbss .sbss.*)
    *(.bss .bss.*)
  }

  PROVIDE(end = .);
}
