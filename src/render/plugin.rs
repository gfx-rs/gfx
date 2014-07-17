// Copyright 2014 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


extern crate rustc;
extern crate syntax;

use std::gc::Gc;
use self::syntax::{ast, ext};
use self::syntax::ext::deriving::generic;
use self::syntax::codemap::Span;
use self::syntax::parse::token;
use self::rustc::plugin::Registry;

use device::shade::ProgramMeta;

fn expand_shader_param(context: &mut ext::base::ExtCtxt, span: Span,
    	meta_item: Gc<ast::MetaItem>, item: Gc<ast::Item>, push: |Gc<ast::Item>|) {
	let trait_def = generic::TraitDef {
		span: span,
		attributes: Vec::new(),
		path: generic::ty::Path::new(vec!("gfx", "ShaderParam")),
		additional_bounds: Vec::new(),
		generics: generic::ty::LifetimeBounds::empty(),
		methods: Vec::new(),
	};
	trait_def.expand(context, meta_item, item, push);
}

#[plugin_registrar]
pub fn registrar(reg: &mut Registry) {
    reg.register_syntax_extension(token::intern("shader_param"),
        ext::base::ItemDecorator(expand_shader_param));
}


pub type UniformLoc = u16;

struct Uploader;

impl Uploader {
	pub fn put_uniform_i32(&mut self, _loc: UniformLoc, _value: i32) {

	}
}

trait ShaderParam {
	fn create(meta: &ProgramMeta) -> Self;
	fn upload(&self, uploader: &mut Uploader);
}
