use core::ffi::CStr;
use core::mem::transmute;
use core::ptr::null_mut;
use core::ptr::NonNull;
use core::sync::atomic::AtomicPtr;
use core::sync::atomic::Ordering;

use ogl33::*;

mod storage {
    use super::*;
    pub static AlphaFunc: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
}

/// Sets the color to clear to when clearing the screen.
pub fn clear_color(r: f32, g: f32, b: f32, a: f32) {
    unsafe {
        glClearColor(r, g, b, a);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    }
}

pub struct VertexArray(pub GLuint);
impl VertexArray {
    /// Creates a new vertex array object
    pub fn new() -> Option<Self> {
        let mut vao = 0;
        unsafe { glGenVertexArrays(1, &mut vao) };
        if vao != 0 {
            Some(Self(vao))
        } else {
            None
        }
    }

    /// Bind this vertex array as the current vertex array object
    pub fn bind(&self) {
        unsafe { glBindVertexArray(self.0) }
    }

    /// Clear the current vertex array object binding.
    pub fn clear_binding() {
        unsafe { glBindVertexArray(0) }
    }
}

/// The types of buffer object that you can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferType {
    /// Array Buffers holds arrays of vertex data for drawing.
    Array = GL_ARRAY_BUFFER as isize,
    /// Element Array Buffers hold indexes of what vertexes to use for drawing.
    ElementArray = GL_ELEMENT_ARRAY_BUFFER as isize,
}

/// Basic wrapper for a [Buffer
/// Object](https://www.khronos.org/opengl/wiki/Buffer_Object).
pub struct Buffer(pub GLuint);
impl Buffer {
    /// Makes a new vertex buffer
    pub fn new() -> Option<Self> {
        let mut vbo = 0;
        unsafe {
            glGenBuffers(1, &mut vbo);
        }
        if vbo != 0 {
            Some(Self(vbo))
        } else {
            None
        }
    }

    /// Bind this vertex buffer for the given type
    pub fn bind(&self, ty: BufferType) {
        unsafe { glBindBuffer(ty as GLenum, self.0) }
    }

    /// Clear the current vertex buffer binding for the given type.
    pub fn clear_binding(ty: BufferType) {
        unsafe { glBindBuffer(ty as GLenum, 0) }
    }
}

/// Places a slice of data into a previously-bound buffer.
pub fn buffer_data(ty: BufferType, data: &[u8], usage: GLenum) {
    unsafe {
        glBufferData(
            ty as GLenum,
            data.len().try_into().unwrap(),
            data.as_ptr().cast(),
            usage,
        );
    }
}

pub fn buffer_sub_data(ty: BufferType, data: &[u8]) {
    unsafe {
        glBufferSubData(
            ty as GLenum,
            0,
            data.len().try_into().unwrap(),
            data.as_ptr().cast(),
        );
    }
}

/// The types of shader object.
pub enum ShaderType {
    /// Vertex shaders determine the position of geometry within the screen.
    Vertex = GL_VERTEX_SHADER as isize,
    /// Fragment shaders determine the color output of geometry.
    ///
    /// Also other values, but mostly color.
    Fragment = GL_FRAGMENT_SHADER as isize,

    // geom shaders
    Geometry = GL_GEOMETRY_SHADER as isize,

    Compute = 0x91B9 as isize,
}

/// A handle to a [Shader
/// Object](https://www.khronos.org/opengl/wiki/GLSL_Object#Shader_objects)
pub struct Shader(pub GLuint);
impl Shader {
    /// Makes a new shader.
    ///
    /// Prefer the [`Shader::from_source`](Shader::from_source) method.
    ///
    /// Possibly skip the direct creation of the shader object and use
    /// [`ShaderProgram::from_vert_frag`](ShaderProgram::from_vert_frag).
    pub fn new(ty: ShaderType) -> Option<Self> {
        let shader = unsafe { glCreateShader(ty as GLenum) };
        if shader != 0 {
            Some(Self(shader))
        } else {
            None
        }
    }

    /// Assigns a source string to the shader.
    ///
    /// Replaces any previously assigned source.
    pub fn set_source(&self, src: &str) {
        unsafe {
            glShaderSource(
                self.0,
                1,
                &(src.as_bytes().as_ptr().cast()),
                &(src.len().try_into().unwrap()),
            );
        }
    }

    /// Compiles the shader based on the current source.
    pub fn compile(&self) {
        unsafe { glCompileShader(self.0) };
    }

    /// Checks if the last compile was successful or not.
    pub fn compile_success(&self) -> bool {
        let mut compiled = 0;
        unsafe { glGetShaderiv(self.0, GL_COMPILE_STATUS, &mut compiled) };
        compiled == i32::from(GL_TRUE)
    }

    /// Gets the info log for the shader.
    ///
    /// Usually you use this to get the compilation log when a compile failed.
    pub fn info_log(&self) -> String {
        let mut needed_len = 0;
        unsafe { glGetShaderiv(self.0, GL_INFO_LOG_LENGTH, &mut needed_len) };
        let mut v: Vec<u8> = Vec::with_capacity(needed_len.try_into().unwrap());
        let mut len_written = 0_i32;
        unsafe {
            glGetShaderInfoLog(
                self.0,
                v.capacity().try_into().unwrap(),
                &mut len_written,
                v.as_mut_ptr().cast(),
            );
            v.set_len(len_written.try_into().unwrap());
        }
        String::from_utf8_lossy(&v).into_owned()
    }

    /// Marks a shader for deletion.
    ///
    /// Note: This _does not_ immediately delete the shader. It only marks it for
    /// deletion. If the shader has been previously attached to a program then the
    /// shader will stay allocated until it's unattached from that program.
    pub fn delete(self) {
        unsafe { glDeleteShader(self.0) };
    }

    /// Takes a shader type and source string and produces either the compiled
    /// shader or an error message.
    ///
    /// Prefer [`ShaderProgram::from_vert_frag`](ShaderProgram::from_vert_frag),
    /// it makes a complete program from the vertex and fragment sources all at
    /// once.
    pub fn from_source(ty: ShaderType, source: &str) -> Result<Self, String> {
        let id = Self::new(ty).ok_or_else(|| "Couldn't allocate new shader".to_string())?;
        id.set_source(source);
        id.compile();
        if id.compile_success() {
            Ok(id)
        } else {
            let out = id.info_log();
            id.delete();
            Err(out)
        }
    }
}

/// A handle to a [Program
/// Object](https://www.khronos.org/opengl/wiki/GLSL_Object#Program_objects)
#[derive(Clone)]
pub struct ShaderProgram(pub GLuint);
impl ShaderProgram {
    /// Allocates a new program object.
    ///
    /// Prefer [`ShaderProgram::from_vert_frag`](ShaderProgram::from_vert_frag),
    /// it makes a complete program from the vertex and fragment sources all at
    /// once.
    pub fn new() -> Option<Self> {
        let prog = unsafe { glCreateProgram() };
        if prog != 0 {
            Some(Self(prog))
        } else {
            None
        }
    }

    /// Attaches a shader object to this program object.
    pub fn attach_shader(&self, shader: &Shader) {
        unsafe { glAttachShader(self.0, shader.0) };
    }

    /// Links the various attached, compiled shader objects into a usable program.
    pub fn link_program(&self) {
        unsafe { glLinkProgram(self.0) };
    }

    /// Checks if the last linking operation was successful.
    pub fn link_success(&self) -> bool {
        let mut success = 0;
        unsafe { glGetProgramiv(self.0, GL_LINK_STATUS, &mut success) };
        success == i32::from(GL_TRUE)
    }

    /// Gets the log data for this program.
    ///
    /// This is usually used to check the message when a program failed to link.
    pub fn info_log(&self) -> String {
        let mut needed_len = 0;
        unsafe { glGetProgramiv(self.0, GL_INFO_LOG_LENGTH, &mut needed_len) };
        let mut v: Vec<u8> = Vec::with_capacity(needed_len.try_into().unwrap());
        let mut len_written = 0_i32;
        unsafe {
            glGetProgramInfoLog(
                self.0,
                v.capacity().try_into().unwrap(),
                &mut len_written,
                v.as_mut_ptr().cast(),
            );
            v.set_len(len_written.try_into().unwrap());
        }
        String::from_utf8_lossy(&v).into_owned()
    }

    /// Sets the program as the program to use when drawing.
    pub fn use_program(&self) {
        unsafe { glUseProgram(self.0) };
    }

    /// Marks the program for deletion.
    ///
    /// Note: This _does not_ immediately delete the program. If the program is
    /// currently in use it won't be deleted until it's not the active program.
    /// When a program is finally deleted and attached shaders are unattached.
    pub fn delete(self) {
        unsafe { glDeleteProgram(self.0) };
    }

    /// Takes a vertex shader source string and a fragment shader source string
    /// and either gets you a working program object or gets you an error message.
    ///
    /// This is the preferred way to create a simple shader program in the common
    /// case. It's just less error prone than doing all the steps yourself.
    pub fn from_vert_frag(vert: &str, frag: &str) -> Result<Self, String> {
        let p = Self::new().ok_or_else(|| "Couldn't allocate a program".to_string())?;
        let v = Shader::from_source(ShaderType::Vertex, vert)
            .map_err(|e| format!("Vertex Compile Error: {}", e))?;
        let f = Shader::from_source(ShaderType::Fragment, frag)
            .map_err(|e| format!("Fragment Compile Error: {}", e))?;
        p.attach_shader(&v);
        p.attach_shader(&f);
        p.link_program();
        v.delete();
        f.delete();
        if p.link_success() {
            Ok(p)
        } else {
            let out = format!("Program Link Error: {}", p.info_log());
            p.delete();
            Err(out)
        }
    }

    pub fn from_compute(compute: &str) -> Result<Self, String> {
        let p = Self::new().ok_or_else(|| "Couldn't allocate a program".to_string())?;
        let c = Shader::from_source(ShaderType::Compute, compute)
            .map_err(|e| format!("Compute Compile Error: {}", e))?;
        p.attach_shader(&c);
        p.link_program();
        c.delete();
        if p.link_success() {
            Ok(p)
        } else {
            let out = format!("Program Link Error: {}", p.info_log());
            p.delete();
            Err(out)
        }
    }

    pub fn from_vert_geom_frag(vert: &str, geom: &str, frag: &str) -> Result<Self, String> {
        let p = Self::new().ok_or_else(|| "Couldn't allocate a program".to_string())?;
        let v = Shader::from_source(ShaderType::Vertex, vert)
            .map_err(|e| format!("Vertex Compile Error: {}", e))?;
        let g = Shader::from_source(ShaderType::Geometry, geom)
            .map_err(|e| format!("Geometry Compile Error: {}", e))?;
        let f = Shader::from_source(ShaderType::Fragment, frag)
            .map_err(|e| format!("Fragment Compile Error: {}", e))?;
        p.attach_shader(&v);
        p.attach_shader(&g);
        p.attach_shader(&f);
        p.link_program();
        v.delete();
        g.delete();
        f.delete();
        if p.link_success() {
            Ok(p)
        } else {
            let out = format!("Program Link Error: {}", p.info_log());
            p.delete();
            Err(out)
        }
    }

    pub fn set_uniform_color(&self, name: &str, value: [GLfloat; 4]) {
        self.use_program();
        unsafe {
            let loc = glGetUniformLocation(self.0, name.as_ptr() as *const i8);
            glUniform4f(loc, value[0], value[1], value[2], value[3]);
        }
    }

    pub fn set_uniform_int(&self, name: &str, value: GLint) {
        self.use_program();
        unsafe {
            let loc = glGetUniformLocation(self.0, name.as_ptr() as *const i8);
            glUniform1i(loc, value);
        }
    }

    pub fn set_uniform_tex(&self, name: &str, value: GLint) {
        self.use_program();
        unsafe {
            let loc = glGetUniformLocation(self.0, name.as_ptr() as *const i8);
            glUniform1i(loc, value);
        }
    }

    pub fn set_uniform_float(&self, name: &str, value: f32) {
        self.use_program();
        unsafe {
            let loc = glGetUniformLocation(self.0, name.as_ptr() as *const i8);
            glUniform1f(loc, value);
        }
    }
}

pub struct Texture {
    pub id: GLuint,
}

impl Texture {
    pub unsafe fn new() -> Self {
        let mut id: GLuint = 0;
        glGenTextures(1, &mut id);
        Self { id }
    }

    pub fn delete(&mut self) {
        unsafe {
            glDeleteTextures(1, [self.id].as_ptr());
        }
    }

    pub unsafe fn bind(&self) {
        glBindTexture(GL_TEXTURE_2D, self.id)
    }
}

type OptVoidPtr = Option<NonNull<c_void>>;

fn fn_ptr_ok(p: *const c_void) -> bool {
    let p_u = p as usize;
    (p_u >= 8) && (p_u != (-1_isize) as usize)
}

unsafe fn meta_loader(
    loader: &mut dyn FnMut(*const c_char) -> *const c_void,
    names: &[&[u8]],
) -> OptVoidPtr {
    for name in names.iter() {
        debug_assert!(*name.iter().last().unwrap() == 0_u8);
        let p = loader(name.as_ptr() as *const c_char);
        if fn_ptr_ok(p) {
            return NonNull::new(p as *mut c_void);
        }
    }
    None
}
#[doc(hidden)]
pub mod BlendFunci {
    use super::*;
    pub static sBlendFunci: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

    #[inline]
    pub fn is_loaded() -> bool {
        !sBlendFunci.load(Ordering::Relaxed).is_null()
    }

    /// Load `glBlendFunci` using the provided loader.
    ///
    /// ## Safety
    /// As per [`load_gl_with`](crate::load_gl_with)
    pub unsafe fn load_with<F>(mut load_fn: F)
    where
        F: FnMut(*const c_char) -> *const c_void,
    {
        if let Some(p) = meta_loader(&mut load_fn, &[b"glBlendFunci\0"]) {
            sBlendFunci.store(p.as_ptr(), Ordering::SeqCst);
        };
    }
}

pub mod MemoryBarrier {
    use super::*;
    pub static sMemoryBarrier: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

    #[inline]
    pub fn is_loaded() -> bool {
        !sMemoryBarrier.load(Ordering::Relaxed).is_null()
    }

    /// Load `glBlendFunci` using the provided loader.
    ///
    /// ## Safety
    /// As per [`load_gl_with`](crate::load_gl_with)
    pub unsafe fn load_with<F>(mut load_fn: F)
    where
        F: FnMut(*const c_char) -> *const c_void,
    {
        if let Some(p) = meta_loader(&mut load_fn, &[b"glMemoryBarrier\0"]) {
            sMemoryBarrier.store(p.as_ptr(), Ordering::SeqCst);
        };
    }
}

pub mod BindImageTexture {
    use super::*;
    pub static sBindImageTexture: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

    #[inline]
    pub fn is_loaded() -> bool {
        !sBindImageTexture.load(Ordering::Relaxed).is_null()
    }

    /// Load `glBlendFunci` using the provided loader.
    ///
    /// ## Safety
    /// As per [`load_gl_with`](crate::load_gl_with)
    pub unsafe fn load_with<F>(mut load_fn: F)
    where
        F: FnMut(*const c_char) -> *const c_void,
    {
        if let Some(p) = meta_loader(&mut load_fn, &[b"glBindImageTexture\0"]) {
            sBindImageTexture.store(p.as_ptr(), Ordering::SeqCst);
        };
    }
}

pub mod DispatchCompute {
    use super::*;
    pub static sDispatchCompute: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

    #[inline]
    pub fn is_loaded() -> bool {
        !sDispatchCompute.load(Ordering::Relaxed).is_null()
    }

    /// Load `glBlendFunci` using the provided loader.
    ///
    /// ## Safety
    /// As per [`load_gl_with`](crate::load_gl_with)
    pub unsafe fn load_with<F>(mut load_fn: F)
    where
        F: FnMut(*const c_char) -> *const c_void,
    {
        if let Some(p) = meta_loader(&mut load_fn, &[b"glDispatchCompute\0"]) {
            sDispatchCompute.store(p.as_ptr(), Ordering::SeqCst);
        };
    }
}

#[inline]
pub unsafe fn glBlendFunci(idx: GLuint, sfactor: GLenum, dfactor: GLenum) {
    let p: *mut c_void = {
        let temp_p = BlendFunci::sBlendFunci.load(Ordering::Relaxed);
        if temp_p.is_null() {
            panic!("glBlendFunci not loaded");
        }
        temp_p
    };
    let out = transmute::<*mut c_void, extern "system" fn(GLuint, GLenum, GLenum)>(p)(
        idx, sfactor, dfactor,
    );
    #[cfg(all(debug_assertions, feature = "debug_error_checks"))]
    {
        let gle = GlError::from(glGetError());
        if gle.needs_reporting() {
            error!("glBlendFunci({:?}, {:?}): {:?}", sfactor, dfactor, gle);
        }
    }
    out
}

#[inline]
pub unsafe fn glDispatchCompute(x: GLuint, y: GLuint, z: GLuint) {
    let p: *mut c_void = {
        let temp_p = DispatchCompute::sDispatchCompute.load(Ordering::Relaxed);
        if temp_p.is_null() {
            panic!("glDispatchCompute not loaded");
        }
        temp_p
    };
    let out = transmute::<*mut c_void, extern "system" fn(GLuint, GLuint, GLuint)>(p)(x, y, z);
    #[cfg(all(debug_assertions, feature = "debug_error_checks"))]
    {
        let gle = GlError::from(glGetError());
        if gle.needs_reporting() {
            error!("glBlendFunci({:?}, {:?}): {:?}", sfactor, dfactor, gle);
        }
    }
    out
}

#[inline]
pub unsafe fn glMemoryBarrier(x: GLuint) {
    let p: *mut c_void = {
        let temp_p = MemoryBarrier::sMemoryBarrier.load(Ordering::Relaxed);
        if temp_p.is_null() {
            panic!("glMemoryBarrier not loaded");
        }
        temp_p
    };
    let out = transmute::<*mut c_void, extern "system" fn(GLuint)>(p)(x);
    #[cfg(all(debug_assertions, feature = "debug_error_checks"))]
    {
        let gle = GlError::from(glGetError());
        if gle.needs_reporting() {
            error!("glBlendFunci({:?}, {:?}): {:?}", sfactor, dfactor, gle);
        }
    }
    out
}

#[inline]
pub unsafe fn glBindImageTexture(
    unit: GLuint,
    tex: GLuint,
    level: GLint,
    layered: GLboolean,
    layer: GLint,
    access: GLenum,
    format: GLenum,
) {
    let p: *mut c_void = {
        let temp_p = BindImageTexture::sBindImageTexture.load(Ordering::Relaxed);
        if temp_p.is_null() {
            panic!("glBindImageTexture not loaded");
        }
        temp_p
    };
    let out = transmute::<
        *mut c_void,
        extern "system" fn(GLuint, GLuint, GLint, GLboolean, GLint, GLenum, GLenum),
    >(p)(unit, tex, level, layered, layer, access, format);
    #[cfg(all(debug_assertions, feature = "debug_error_checks"))]
    {
        let gle = GlError::from(glGetError());
        if gle.needs_reporting() {
            error!("glBlendFunci({:?}, {:?}): {:?}", sfactor, dfactor, gle);
        }
    }
    out
}
