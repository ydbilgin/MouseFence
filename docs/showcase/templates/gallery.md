@grid gallery columns=2
---
<table>
{{#gallery_rows}}
<tr>
{{#cells}}
<td width="{{gallery_cell_width}}%" valign="top">
{{^filler}}
<picture>
{{#dark}}
  <source media="(prefers-color-scheme: dark)" srcset="{{dark}}">
{{/dark}}
  <img src="{{file}}" alt="{{alt}}" width="100%">
</picture>
<br><sub>{{caption}}</sub>
{{/filler}}
</td>
{{/cells}}
</tr>
{{/gallery_rows}}
</table>
