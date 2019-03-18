.PHONY: all
all: frag.spv vert.spv

frag.spv: shader.frag
	glslangValidator.exe -V ./shader.frag

vert.spv: shader.vert
	glslangValidator.exe -V ./shader.vert
