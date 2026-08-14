import MaterialX as mx
import MaterialX.PyMaterialXGenShader as mx_gen_shader
import MaterialX.PyMaterialXGenGlsl as mx_gen_glsl

# 1. Load the standard data library (libraries/)
stdlib = mx.createDocument()
searchPath = mx.getDefaultDataSearchPath()
mx.loadLibraries(mx.getDefaultDataLibraryFolders(), searchPath, stdlib)

# 2. Load the material document (the Open Chess Set material)
doc = mx.createDocument()
mx.readFromXmlFile(doc, "resources/Materials/Examples/StandardSurface/standard_surface_chess_set.mtlx", searchPath)
doc.setDataLibrary(stdlib)

# 3. Create the GLSL shader generator and its generation context
gen = mx_gen_glsl.GlslShaderGenerator.create()
context = mx_gen_shader.GenContext(gen)
context.registerSourceCodeSearchPath(searchPath)  # for libraries/stdlib/genglsl/*.glsl
gen.registerTypeDefs(doc)

# 4. Generate a shader for the first renderable element (e.g. the chessboard)
elems = mx_gen_shader.findRenderableElements(doc)
print("renderable elements:", [e.getName() for e in elems])
shader = gen.generate("M_Chessboard", elems[0], context)

# 5. Print the generated source code
print("---- vertex shader (first 400 chars) ----")
print(shader.getSourceCode(mx_gen_shader.VERTEX_STAGE)[:400])
print("---- pixel shader (first 400 chars) ----")
print(shader.getSourceCode(mx_gen_shader.PIXEL_STAGE)[:400])
