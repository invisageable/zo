use super::parser::Parser;

use zo_ast::ast::{Expr, ExprKind};
use zo_ast::ast_zsx::{Attr, AttrKind, Html, Name, Tag, TagKind};
use zo_interner::interner::symbol::Symbolize;
use zo_reporter::Result;
use zo_tokenizer::token::group::Group;
use zo_tokenizer::token::punctuation::Punctuation;
use zo_tokenizer::token::{Token, TokenKind};

use swisskit::span::Span;

use thin_vec::ThinVec;

impl<'tokens> Parser<'tokens> {
  pub(crate) fn parse_zsx(&mut self) -> Result<Expr> {
    let mut state = State::Idle;
    let mut attrs = ThinVec::with_capacity(0usize);
    // let mut text = ThinVec::with_capacity(0usize);
    let mut current_span = self.current_span();

    loop {
      match state {
        State::Idle => {
          state = State::TagStart;
        }
        State::TagStart => {
          let token = self.maybe_token_current.unwrap_or(&Token::EOF);

          println!("{:?}", self.maybe_token_current);
          println!("{:?}", self.maybe_token_next);

          match token.kind {
            TokenKind::Punctuation(Punctuation::LessThan) => {
              state = State::TagName;

              self.next();
            }
            TokenKind::Punctuation(Punctuation::Slash) => {
              state = State::TagEnd;

              self.next();
            }
            _ => panic!(),
          }
        }
        State::TagName => {
          let token = self.maybe_token_current.unwrap_or(&Token::EOF);

          match token.kind {
            TokenKind::Punctuation(Punctuation::GreaterThan) => {
              state = State::TagData;

              self.next();
            }
            TokenKind::Punctuation(Punctuation::Slash) => {
              state = State::TagEnd;

              self.next();
            }
            TokenKind::Ident(_) => {
              let expr = Self::parse_expr_lit_ident(self)?;

              state = State::TagAttr;

              self.next();
            }
            _ => panic!(),
          }
        }
        State::TagData => {
          let token = self.maybe_token_current.unwrap_or(&Token::EOF);

          match token.kind {
            TokenKind::Punctuation(Punctuation::LessThan) => {
              state = State::TagStart;

              self.next();
            }
            _ => panic!(),
          }
        }
        State::TagEmpty => todo!(),
        State::TagAttr => {
          let token = self.maybe_token_current.unwrap_or(&Token::EOF);

          match token.kind {
            TokenKind::Punctuation(Punctuation::GreaterThan) => {
              state = State::TagStart;

              self.next();
            }
            TokenKind::Ident(_) => {
              attrs.push(self.parse_attr()?);
              self.next();
            }
            kind => panic!("State::TagAttr — `{kind}`"),
          }
        }
        State::TagEnd => {
          let token = self.maybe_token_current.unwrap_or(&Token::EOF);
          let mut name = None;

          match token.kind {
            TokenKind::Punctuation(Punctuation::GreaterThan) => {
              name = None;

              // returns a tag fragment such as `<></>`.

              self.next();

              return Ok(Expr {
                kind: ExprKind::Tag(Tag {
                  kind: TagKind::Fragment,
                  attrs,
                }),
                span: Span::merge(current_span, self.current_span()),
              });
            }
            TokenKind::Ident(_) => {
              name = Some(Self::parse_expr_lit_ident(self)?);

              self.next();

              if self
                .ensure_peek(TokenKind::Punctuation(Punctuation::Semicolon))
              {
                return Ok(Expr {
                  kind: ExprKind::Tag(Tag {
                    kind: TagKind::Name(match name {
                      Some(name) => {
                        let name = self.interner.lookup(**name.as_symbol());

                        Name::Html(Html::from(name))
                      }
                      None => Name::Custom(Box::new(name.unwrap())),
                    }),
                    attrs,
                  }),
                  span: Span::merge(current_span, self.current_span()),
                });
              }
            }
            _ => panic!(),
          }
        }
      }
    }
  }

  fn parse_attr(&mut self) -> Result<Attr> {
    let token = self.maybe_token_current.unwrap_or(&Token::EOF);

    match token.kind {
      TokenKind::Ident(_) => {
        let name = Self::parse_expr_lit_ident(self)?;
        let mut value = None;

        if self.ensure_peek(TokenKind::Punctuation(Punctuation::Equal)) {
          self.next();
          self.next();

          value = Some(Self::parse_expr_lit_ident(self)?);

          println!("{:?}", self.maybe_token_current);
          println!("{:?}", self.maybe_token_next);
        }

        // if self
        //   .ensure_peek(TokenKind::Punctuation(Punctuation::GreaterThan))
        //   .is_ok()
        // {
        //   // state = State::TagEnd;
        // }

        // println!("{:?}", self.maybe_token_current);
        // println!("{:?}", self.maybe_token_next);

        Ok(Attr {
          kind: AttrKind::Static(Box::new(name), value),
        })
      }
      TokenKind::Group(Group::BraceOpen) => todo!(),
      kind => panic!("{kind}"),
    }
  }
}

pub enum State {
  Idle,
  TagStart,
  TagName,
  TagData,
  TagAttr,
  TagEmpty,
  TagEnd,
}
