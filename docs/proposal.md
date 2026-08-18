# Dowel — 企画書

«A Rust-powered universal UI compiler and accessibility-first layer for React Native.»

---

## 1. 概要

Dowel は、React Native によるクロスプラットフォーム UI 開発を、Rust 製コンパイラによって Web / Native それぞれに適した形へ変換・最適化するための基盤である。

新規プロジェクトでは `@dowel/core` を使うことで、React Native / Web / Tailwind / accessibility を個別に組み合わせることなく、最初から統合された開発環境を利用できる。

一方、既存の React Native / React Native for Web プロジェクトに対しては、`@dowel/compiler` を追加することで、既存コードを大きく書き換えることなく段階的に Dowel の最適化を導入できる。

Dowel は新しいフルスタック UI フレームワークを作ることを目的としない。

目指すのは、

«既存の React Native ecosystem を尊重しながら、その下に高速で薄い compilation layer を提供すること»

である。

命名の由来: 木工におけるダボ継ぎ（dowel joint）。複数の部材を外から目立たない形で正確に接続する技法。

---

## 2. 基本思想

Dowel の設計は、以下の4原則を中心に置く。

### 2.1 Existing source first

既存プロジェクトに Dowel 専用 API への全面移行を要求しない。

React Native のコードは、そのまま Dowel compiler の入力として利用できる。

```tsx
import { View, Text } from 'react-native'

export function Card() {
  return (
    <View className="rounded-xl p-4">
      <Text className="font-bold">
        Hello
      </Text>
    </View>
  )
}
```

Dowel compiler はこのコードを解析し、安全に変換できる部分だけを最適化する。

---

### 2.2 Golden path for new projects

既存コードを尊重する一方、新規プロジェクトではセットアップの複雑さそのものを減らしたい。

そのため `@dowel/core` を公式の推奨 entry point とする。

```tsx
import { View, Text, Paragraph, Heading, Section, Button } from '@dowel/core'
```

`@dowel/core` は巨大な UI framework ではなく、

- View
- Text
- Paragraph（Webの `p` / Nativeの `Text`）
- Heading（Webの `h1`〜`h6` / Nativeのheader role付き`Text`）
- Section（Webの `section` / Nativeの `View`）
- Pressable
- Button
- Link

などの canonical primitives と semantic primitives を提供する薄いレイヤーである。

新規プロジェクトでは、

```
npm create dowel@latest
```

から、

```
npm run web
npm run ios
npm run android
```

までを最短距離で成立させることを目標とする。

Dowel Core の役割は、Dowel 利用を必須化することではなく、

«最も設定が少なく、最も最適化しやすく、最も accessibility が保証される経路»

を提供することである。

---

### 2.3 Compile what you can, fall back gracefully

Dowel は100%の静的変換を前提としない。

コンパイラが安全に理解できる部分はビルド時に変換し、理解できない部分は既存 runtime に委譲する。

```
                  React Native source
                         │
                         ▼
                  Dowel analysis
                         │
                ┌────────┴────────┐
                │                 │
           understood         unsupported
                │                 │
             lowering          fallback
                │                 │
                └────────┬────────┘
                         │
                      runtime
```

fallback は失敗ではなく、Dowel の正式な設計要素とする。

Dowel の成熟に従って、静的に扱える coverage を徐々に増やしていく。

---

### 2.4 Accessibility is not optional

Accessibility は追加機能ではなく、Dowel の基本仕様とする。

v1 から、

- semantic HTML
- React Native accessibility props
- compile-time diagnostics
- keyboard interaction
- focus management

を設計対象に含める。

Dowel において accessibility を「後から付ける」状態は作らない。

---

## 3. なぜ Dowel が必要か

### 3.1 React Native for Web のセットアップは強力だが複数レイヤーに分かれている

React Native で Web / iOS / Android を共通化する場合、実際には複数のツールを組み合わせる必要がある。

例えば、

```
React Native
+
React Native for Web
+
styling solution
+
Tailwind integration
+
Metro / Babel configuration
+
Web bundler integration
+
accessibility implementation
```

といった構成になる。

個々のツールは優れているが、新規プロジェクトを作るたびに統合方法を理解・設定する必要がある。

Dowel Core はこの組み合わせを一つの推奨構成として提供する。

---

### 3.2 静的に分かる情報まで runtime に残ることがある

React / React Native のクロスプラットフォーム stack では、

- style resolution
- conditional styles
- platform differences
- semantic mapping
- component wrappers

などが runtime で処理される場合がある。

Dowel は source code 全体を compiler から見ることで、

«本当に runtime で必要な処理だけを runtime に残す»

ことを目指す。

設計原則は、

«Pay runtime cost only for what is genuinely dynamic.»

とする。

---

### 3.3 Web と Native は同じではない

Web を React Native の単純なエミュレーションとして扱わない。

同じ source component から、

```
                  Dowel IR
                 /        \
                /          \
              Web          Native
               │              │
        DOM / CSS / ARIA   React Native
                           primitives
```

へ platform-specific lowering を行う。

Web では Web の semantic primitive を優先する。

Native では React Native / Fabric の ecosystem をそのまま利用する。

---

## 4. アーキテクチャ

```
                       Application
                           │
             ┌─────────────┴─────────────┐
             │                           │
        @dowel/core                Existing RN code
        recommended                     │
             │                          │
             └─────────────┬────────────┘
                           │
                           ▼
                    Dowel Compiler
                      Rust core
                           │
             ┌─────────────┼─────────────┐
             │             │             │
          Style IR    Semantic IR    Diagnostics
             │             │             │
             └─────────────┼─────────────┘
                           │
                       Dowel IR
                           │
             ┌─────────────┴─────────────┐
             │                           │
         Web backend                Native backend
             │                           │
        DOM + CSS                  React Native
        semantic HTML              View / Text
        ARIA                       StyleSheet
             │                     accessibility props
             │
         fallback
             │
             RNW
```

---

## 5. Dowel Core

`@dowel/core` は、新規 Dowel project の canonical API を提供する。

初期 primitive は、

```
View
Text
Image
Pressable
Button
Link
```

程度に限定する。

例えば、

```tsx
import { View, Text, Button } from '@dowel/core'

export function Login() {
  return (
    <View className="flex-1 items-center justify-center p-6">
      <Text className="text-xl font-bold">
        Welcome
      </Text>

      <Button className="mt-4 px-4 py-2">
        Continue
      </Button>
    </View>
  )
}
```

Web では可能な限り、

```html
<div>
  <span>Welcome</span>
  <button>Continue</button>
</div>
```

へ直接 lowering する。

Native では、

```
View
Text
Pressable
```

等へ lowering する。

Dowel Core は runtime abstraction を増やすためではなく、

«compiler が意味を最も正確に理解できる canonical source»

を提供するために存在する。

---

## 6. Styling

### 6.1 Tailwind first

Dowel v1 における公式 styling API は Tailwind とする。

独自 CSS DSL は作らない。

```tsx
<View className="flex flex-col gap-4 p-6 md:flex-row">
```

という既に広く知られた記述方法をそのまま利用する。

---

### 6.2 Tailwind は frontend、Dowel IR は内部表現

Dowel 内部を Tailwind 固有構造に固定しない。

```
Tailwind
   │
   ▼
CSS / utility semantics
   │
   ▼
Dowel Style IR
   │
   ▼
platform lowering
```

Dowel Style IR は例えば、

```
Display(Flex)
FlexDirection(Row)
Padding(...)
Gap(...)
BackgroundColor(...)
FontSize(...)
Media(...)
Hover(...)
Focus(...)
Disabled(...)
```

などの platform-independent な情報を保持する。

---

### 6.3 Universal Style Subset

Web / Native の双方で意味を保ったまま利用できる styling 領域を内部的に定義する。

初期対象:

**Layout**

- flex
- direction
- align / justify
- gap
- margin / padding
- width / height
- absolute positioning

**Visual**

- background
- color
- opacity
- border
- border-radius

**Typography**

- font-size
- font-weight
- line-height
- text-align

**Conditions**

- responsive
- hover
- focus
- pressed
- disabled
- platform variants

これはユーザー向けの新しい言語ではない。

Tailwind class を platform capabilities に変換するための内部モデルである。

---

## 7. 動的 className

Dowel は dynamic className を一律 runtime 扱いにはしない。

例えば、

```tsx
<View
  className={cn(
    'p-4',
    active && 'bg-blue-500',
    size === 'lg' && 'text-xl'
  )}
/>
```

について、`active` や `size` の値をコンパイル時に知る必要はない。

式の構造を保持したまま、

```
p-4

active
  → bg-blue-500

size === lg
  → text-xl
```

という conditional style expression に lowering する。

一方、

```tsx
<View className={classNameFromProps} />
```

のように compiler が意味を特定できない場合は、小さな runtime path へ fallback する。

したがって、

```
Static styles
    → compile away

Structurally dynamic styles
    → compile conditional expression

Truly dynamic styles
    → runtime fallback
```

という3段階を基本とする。

---

## 8. Web lowering

Dowel は Web において、React Native primitive を可能な範囲で直接 DOM へ lowering する。

例えば、

```tsx
<View className="p-4">
```

を、

```html
<div class="...">
```

へ変換する。

ただし React Native primitive の semantics が複雑な場合は、無理に lowering しない。

例えば responder system や特殊な interaction behavior が必要な component は、RNW fallback を利用できる。

---

### 8.1 React Native semantics

React Native View には Web の `div` とは異なる default behavior がある。

Dowel は例えば、

```
display: flex
flex-direction: column
flex-shrink: 0
position: relative
min-width: 0
box-sizing: border-box
```

といった View semantics を Dowel IR 内で定義する。

Web lowering 時には shared base style として適用する。

```html
<div class="dowel-view p-4">
```

のように共通ルールとして出力し、重複を避ける。

---

## 9. Native lowering

Native target では React Native ecosystem をそのまま利用する。

Dowel は、

```
Dowel IR
   ↓
React Native primitive
   ↓
Fabric
```

という位置に留まる。

独自 renderer や Fabric の置き換えは行わない。

これにより、

- Expo
- native modules
- React Native libraries
- platform integrations

との互換性を保つ。

---

## 10. Accessibility

Accessibility は v1 から Dowel Core / Compiler / Runtime のすべてに関係する。

### 10.1 Semantic lowering

例えば、

```tsx
<Button disabled={disabled}>
  Save
</Button>
```

を Web では、

```html
<button disabled>
  Save
</button>
```

へ lowering する。

Native では、

```tsx
<Pressable
  disabled={disabled}
  accessibilityRole="button"
  accessibilityState={{ disabled }}
>
```

相当へ lowering する。

原則は、

«Prefer platform semantics over compatibility emulation.»

とする。

---

### 10.2 Compile-time diagnostics

Dowel compiler は accessibility の問題を build 時に検出する。

例えば、

```tsx
<Pressable onPress={save}>
  Save
</Pressable>
```

に対して、

```
warning[DOWEL_A11Y_001]

Interactive Pressable has no accessible role.

Consider:
  accessibilityRole="button"
```

のような diagnostic を出せる。

対象候補:

- interactive element without role
- image without accessible label
- form control without label
- invalid semantic nesting
- inaccessible disabled state
- keyboard-inaccessible Web interaction

---

### 10.3 Runtime accessibility

すべての accessibility 問題を compiler で解決しようとはしない。

以下のような behavior は runtime が担当する。

- focus trap
- focus restoration
- keyboard navigation
- roving tabindex
- Escape handling
- live region timing
- virtual focus

v1 では特に Dialog を最初の高難度 primitive として実装する。

Dialog は、

- initial focus
- focus trap
- focus restoration
- Escape
- modal semantics
- background inert
- screen reader behavior

まで含めて品質基準を定める。

---

## 11. パッケージ構成

```
@dowel/core
    recommended primitives
    semantic components

@dowel/compiler
    Rust compiler
    TSX analysis
    Dowel IR
    Web / Native lowering
    diagnostics

@dowel/runtime
    truly dynamic styles
    interactive behavior
    accessibility behavior

@dowel/tailwind
    Tailwind integration

@dowel/a11y
    complex accessibility primitives
    Dialog etc.
```

将来的に、

```
@dowel/nativewind-compat
@dowel/tamagui-compat
```

のような compatibility layer を追加できる構造にする。

---

## 12. 既存エコシステムとの関係

**React Native**

Native target の基盤として利用する。

既存 React Native project は Dowel Core へ移行せずとも compiler の恩恵を受けられることを目標とする。

---

**React Native for Web**

Web backend における fallback implementation として利用できる。

Dowel が安全に直接 lowering できる領域が増えるほど、application-owned component tree における RNW dependency を減らしていく。

v1 では RNW 完全排除を成功条件にはしない。

---

**NativeWind**

Tailwind を React Native で利用するという developer experience を共有する。

既存 NativeWind project から段階的に Dowel を利用できる compatibility path を検討する。

ただし Dowel Core 自体は NativeWind の bug-for-bug compatibility を仕様とはしない。

---

**Tamagui**

Tamagui ecosystem には豊富な component / styling assets が存在する。

長期的には Tamagui-compatible source を Dowel IR へ変換できる compatibility layer を研究する。

```
Tamagui-compatible source
          │
       understood?
        /       \
      yes        no
       │          │
 Dowel lowering  Tamagui fallback
```

完全互換を最初から要求するのではなく、compiler coverage を段階的に増やす。

---

## 13. ロードマップ

### Phase 0 — Vertical prototype

最初から巨大な framework を作らない。

最小の縦切りを完成させる。

対象:

```
View
Text
Pressable
Button

Tailwind className

flex
spacing
color
typography

conditional className

Web lowering
Native lowering

basic a11y diagnostics
Button semantics
```

確認すること:

- Rust から TSX を十分高速に解析できるか
- Tailwind semantics を Style IR に落とせるか
- dynamic expression を構造のまま lowering できるか
- Web direct lowering と RNW fallback を混在できるか
- Native style output が成立するか
- semantic lowering が Web / Native の双方で成立するか

---

### Phase 1 — Dowel v1

**New project path**

```
npm create dowel@latest
```

から Web / iOS / Android をすぐ起動できる。

`@dowel/core` を利用する。

---

**Existing project path**

`@dowel/compiler` を既存 React Native project に追加できる。

source API は変更しなくてよい。

---

**Styling**

Tailwind first。

static / conditional / runtime fallback の3段階を実装する。

---

**Accessibility**

v1 から必須。

- semantic Button / Link
- compile-time diagnostics
- basic form semantics
- Dialog

を含める。

---

**Web**

安全な component を direct DOM lowering。

未対応 component は RNW fallback。

---

## 14. Phase 2 — Coverage expansion

実際の project で利用しながら、

- supported React Native APIs
- Tailwind utilities
- interaction states
- accessibility primitives
- Web direct lowering coverage

を拡大する。

---

## 15. Phase 3 — Compatibility

需要に応じて、

- NativeWind migration
- Tamagui compatibility
- third-party component analysis

を進める。

---

## 16. Phase 4 — Advanced optimization

Dowel IR を利用して、

- static style extraction
- dead style elimination
- component flattening
- wrapper elimination
- constant folding
- token resolution
- platform dead-code elimination
- semantic element selection

などの最適化を行う。

---

## 17. 成功指標

Dowel の成功を「RNW を完全に消せたか」だけで評価しない。

**Adoption**

- 外部 production project 数
- 新規 Dowel project 数
- 既存 RN project への導入数
- compatibility request 数

**Compiler coverage**

- application-owned RN primitives の direct lowering 率
- runtime fallback 率
- RNW fallback 率

**Performance**

- cold build
- incremental build
- HMR
- production build
- runtime JS size
- bundle size
- style resolution cost

**Web output**

- wrapper 削減数
- DOM node 数
- generated CSS size
- semantic HTML 使用率

**Accessibility**

- automated diagnostic coverage
- semantic HTML coverage
- keyboard test pass rate
- screen reader test coverage
- primitive ごとの accessibility conformance

---

## 18. リスク

| リスク | 方針 |
|---|---|
| compiler scope が膨張する | partial lowering + fallback を正式仕様とする |
| RNW compatibility の再実装が巨大化する | v1 で完全互換を目指さず、安全な subset から始める |
| Tailwind semantics が複雑 | Style IR を境界に置き、Dowel 内部表現を分離する |
| NativeWind compatibility が重い | core 仕様と compatibility layer を分離する |
| dynamic styling が runtime を必要とする | genuinely dynamic なケースに runtime cost を限定する |
| accessibility の保守コストが大きい | primitive を狭くし、品質を coverage より優先する |
| core が framework 化する | canonical primitives に限定し、UI kit を作らない |
| optimization が実用上効かない | Phase 0 から実 project benchmark を取る |

---

## 19. Dowel が目指す位置

Dowel は単なる、

- styling library
- UI kit
- RNW replacement
- accessibility library

のいずれでもない。

これらの境界に位置する。

```
                     React Native source
                            │
             ┌──────────────┴──────────────┐
             │                             │
        @dowel/core                 existing ecosystem
             │                             │
             └──────────────┬──────────────┘
                            │
                      Dowel Compiler
                            │
                         Dowel IR
                            │
             ┌──────────────┴──────────────┐
             │                             │
            Web                          Native
             │                             │
      semantic DOM + CSS              React Native
      accessibility                   accessibility
      minimal runtime                 minimal runtime
```

新規 project では Dowel Core を使えばよい。

既存 project では Dowel Compiler を一つ足せばよい。

複雑な部分は Dowel が下で処理する。

---

## 20. 長期ビジョン

Dowel の理想形は、Dowel 固有 API の利用率が高いことではない。

むしろ、

> **Dowel を意識しなくても、既存 React Native ecosystem がより効率よく Web / Native に接続される状態**

を作ることである。

Dowel Core は最も簡単な入口。

Dowel Compiler は既存 ecosystem への入口。

Dowel IR はその両者を接続する内部基盤。

Accessibility はその全経路に共通する基本要件。

最終的には、

```
React Native ecosystem
          │
          ▼
       Dowel IR
          │
      ┌───┴───┐
      │       │
     Web    Native
```

という共通 compilation substrate を目指す。

Dowel が目立つ必要はない。

複数の部材を外から目立たない形で正確につなぐ。

その役割そのものが **Dowel** という名前の意味である。
