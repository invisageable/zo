---
order: 2
title: Language Design
---

# language design

## keywords

zo has 53 keywords in total:

<div class="keywords">
  <dl class="keywords-list">
    <dt><b>Namespacing</b></dt><dd>4 letters</dd>
    <dt><b class="keyword">pack</b></dt><dd>declares the current package</dd>
    <dt><b class="keyword">load</b></dt><dd>imports items into scope</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Type Definitions</b></dt><dd>misc letters</dd>
    <dt><b class="keyword">abstract</b></dt><dd>declares a behavior contract</dd>
    <dt><b class="keyword">struct</b></dt><dd>declares a record with named fields</dd>
    <dt><b class="keyword">apply</b></dt><dd>attaches behavior to a type</dd>
    <dt><b class="keyword">enum</b></dt><dd>declares a tagged union</dd>
    <dt><b class="keyword">type</b></dt><dd>declares a type alias</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Member Definitions</b></dt><dd>misc letters</dd>
    <dt><b class="keyword">fun</b></dt><dd>declares a function</dd>
    <dt><b class="keyword">ffi</b></dt><dd>declares a foreign function binding</dd>
    <dt><b class="keyword">val</b></dt><dd>declares a compile-time constant</dd>
    <dt><b class="keyword">imu</b></dt><dd>declares an immutable binding</dd>
    <dt><b class="keyword">mut</b></dt><dd>declares a mutable binding</dd>
    <dt><b class="keyword">fn</b></dt><dd>declares a closure</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Control Flow</b></dt><dd>misc letters</dd>
    <dt><b class="keyword">continue</b></dt><dd>skips to the next iteration</dd>
    <dt><b class="keyword">return</b></dt><dd>returns a value from a function</dd>
    <dt><b class="keyword">match</b></dt><dd>pattern matches across arms</dd>
    <dt><b class="keyword">while</b></dt><dd>loops while a condition holds</dd>
    <dt><b class="keyword">break</b></dt><dd>exits the current loop</dd>
    <dt><b class="keyword">else</b></dt><dd>alternate branch of an if</dd>
    <dt><b class="keyword">when</b></dt><dd>ternary expression</dd>
    <dt><b class="keyword">loop</b></dt><dd>infinite loop</dd>
    <dt><b class="keyword">for</b></dt><dd>iterates over a range or collection</dd>
    <dt><b class="keyword">if</b></dt><dd>conditional branch</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Concurrency</b></dt><dd>misc letters</dd>
    <dt><b class="keyword">supervise</b></dt><dd>declares a supervised task scope</dd>
    <dt><b class="keyword">nursery</b></dt><dd>declares a structured task scope</dd>
    <dt><b class="keyword">select</b></dt><dd>multiplexes channels or tasks</dd>
    <dt><b class="keyword">thread</b></dt><dd>marks an OS thread spawn (parser-synthetic)</dd>
    <dt><b class="keyword">spawn</b></dt><dd>launches a concurrent task</dd>
    <dt><b class="keyword">await</b></dt><dd>suspends until a task completes</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Infixes</b></dt><dd>misc letters</dd>
    <dt><b class="keyword">and</b></dt><dd>...</dd>
    <dt><b class="keyword">as</b></dt><dd>casts a value to a type</dd>
    <dt><b class="keyword">is</b></dt><dd>type test (reserved)</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Modifiers</b></dt><dd>misc letters</dd>
    <dt><b class="modifier">group</b></dt><dd>...</dd>
    <dt><b class="modifier">wasm</b></dt><dd>marks an item for WebAssembly</dd>
    <dt><b class="modifier">pub</b></dt><dd>marks an item as public</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Qualifiers</b></dt><dd>4 letters</dd>
    <dt><b class="qualifier">Self</b></dt><dd>refers to the current type</dd>
    <dt><b class="qualifier">self</b></dt><dd>refers to the current instance</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Values</b></dt><dd>misc letters</dd>
    <dt><b class="keyword">false</b></dt><dd>boolean false literal</dd>
    <dt><b class="keyword">true</b></dt><dd>boolean true literal</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Integers</b></dt><dd>misc letters</dd>
    <dt><b class="type">uint</b></dt><dd>the default unsigned integer (32-bit)</dd>
    <dt><b class="type">int</b></dt><dd>the default signed integer (32-bit)</dd>
    <dt><b class="type">s16</b></dt><dd>16-bit signed integer</dd>
    <dt><b class="type">s32</b></dt><dd>32-bit signed integer</dd>
    <dt><b class="type">s64</b></dt><dd>64-bit signed integer</dd>
    <dt><b class="type">u16</b></dt><dd>16-bit unsigned integer</dd>
    <dt><b class="type">u32</b></dt><dd>32-bit unsigned integer</dd>
    <dt><b class="type">u64</b></dt><dd>64-bit unsigned integer</dd>
    <dt><b class="type">s8</b></dt><dd>8-bit signed integer</dd>
    <dt><b class="type">u8</b></dt><dd>8-bit unsigned integer</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Floats</b></dt><dd>misc letters</dd>
    <dt><b class="type">float</b></dt><dd>the default floating-point (64-bit)</dd>
    <dt><b class="type">f32</b></dt><dd>32-bit floating-point</dd>
    <dt><b class="type">f64</b></dt><dd>64-bit floating-point</dd>
  </dl>
  <dl class="keywords-list">
    <dt><b>Primitives</b></dt><dd>misc letters</dd>
    <dt><b class="type">bytes</b></dt><dd>byte buffer</dd>
    <dt><b class="type">bool</b></dt><dd>boolean type</dd>
    <dt><b class="type">char</b></dt><dd>Unicode character</dd>
    <dt><b class="type">str</b></dt><dd>UTF-8 string</dd>
    <dt><b class="type">&lt;/&gt;</b></dt><dd>template fragment type</dd>
    <dt><b class="type">Fn</b></dt><dd>function type</dd>
  </dl>
</div>

## operators

### unary

<table>
  <thead>
    <tr><th>Precedence</th><th>Operator</th></tr>
  </thead>
  <tbody>
    <tr>
      <td>0</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">+</span>
          <span class="operator">-</span>
          <span class="operator">!</span></td>
        </code>
      </tr>
  </tbody>
</table>

### binary

<table>
  <thead>
    <tr>
      <th>Precedence</th>
      <th>Operator</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>1</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">||</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>2</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">&amp;&amp;</span>
          <span class="operator">..</span>
          <span class="operator">..=</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>3</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">==</span>
          <span class="operator">!=</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>4</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">&lt;</span>
          <span class="operator">&lt;=</span>
          <span class="operator">&gt;</span>
          <span class="operator">&gt;=</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>5</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">|</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>6</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">^</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>7</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">&amp;</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>8</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">&lt;&lt;</span>
          <span class="operator">&gt;&gt;</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>9</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">+</span>
          <span class="operator">++</span>
          <span class="operator">-</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>10</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">*</span>
          <span class="operator">/</span>
          <span class="operator">%</span>
        </code>
      </td>
    </tr>
    <tr>
      <td>12</td>
      <td>
        <code class="f-iosevka">
          <span class="operator">.</span>
        </code>
      </td>
    </tr>
  </tbody>
</table>

### assignments

<table>
  <thead>
    <tr>
      <th>Operator</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">=</span>
          <span class="operator">+=</span>
          <span class="operator">-=</span>
          <span class="operator">*=</span>
          <span class="operator">/=</span>
          <span class="operator">%=</span>
          <span class="operator">&amp;=</span>
          <span class="operator">|=</span>
          <span class="operator">^=</span>
          <span class="operator">&lt;&lt;=</span>
          <span class="operator">&gt;&gt;=</span>
          <span class="operator">:=</span>
          <span class="operator">::=</span>
        </code>
      </td>
    </tr>
  </tbody>
</table>

### others

<table>
  <thead>
    <tr>
      <th>Operator</th>
      <th>Name</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">-&gt;</span>
        </code>
      </td>
      <td>arrow (return type)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">=&gt;</span>
        </code>
      </td>
      <td>fat arrow (match arm)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">=:&gt;</span>
        </code>
      </td>
      <td>template fat arrow (template-body closure)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">|&gt;</span>
        </code>
      </td>
      <td>pipe arrow (reserved)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">::</span>
        </code>
      </td>
      <td>path separator</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">?</span>
        </code>
      </td>
      <td>question</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">@</span>
        </code>
      </td>
      <td>at</td>
    </tr>
  </tbody>
</table>

### delimiters

<table class="delimiters">
  <thead>
    <tr>
      <th>Open</th>
      <th>Close</th>
      <th>Name</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">(</span>
        </code>
      </td>
      <td>
        <code class="f-iosevka">
          <span class="operator">)</span>
        </code>
      </td>
      <td>parentheses</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">{</span>
        </code>
      </td>
      <td>
        <code class="f-iosevka">
          <span class="operator">}</span>
        </code>
      </td>
      <td>braces</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">[</span>
        </code>
      </td>
      <td>
        <code class="f-iosevka">
          <span class="operator">]</span>
        </code>
      </td>
      <td>brackets</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">&lt;&gt;</span>
        </code>
      </td>
      <td>
        <code class="f-iosevka">
          <span class="operator">&lt;/&gt;</span>
        </code>
      </td>
      <td>template fragment</td>
    </tr>
  </tbody>
</table>

### punctuations

<table>
  <thead>
    <tr>
      <th>Symbol</th>
      <th>Name</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">,</span>
        </code>
      </td>
      <td>comma (list separator)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">;</span>
        </code>
      </td>
      <td>semicolon (statement terminator)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">:</span>
        </code>
      </td>
      <td>colon (type ascription)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">_</span>
        </code>
      </td>
      <td>underscore (wildcard pattern)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">#</span>
        </code>
      </td>
      <td>hash (reserved)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">$</span>
        </code>
      </td>
      <td>dollar (reserved)</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">%%</span>
        </code>
      </td>
      <td>attribute marker</td>
    </tr>
    <tr>
      <td>
        <code class="f-iosevka">
          <span class="operator">...</span>
        </code>
      </td>
      <td>ellipsis (spread / variadic)</td>
    </tr>
  </tbody>
</table>
