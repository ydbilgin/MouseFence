<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/media/banner-dark.svg">
    <img src="docs/media/banner-light.svg" alt="{{name}}: {{tagline}}" width="100%">
  </picture>
</p>

<p align="center">{{description}}</p>

<p align="center">
{{#badges}}
  {{#link}}<a href="{{link}}">{{/link}}<img alt="{{alt}}" src="{{image}}">{{#link}}</a>{{/link}}
{{/badges}}
</p>

{{#hero}}
<p align="center">
  <img src="{{media}}" alt="{{alt}}" width="800">
{{#caption}}
  <br><sub><i>{{caption}}</i></sub>
{{/caption}}
</p>
{{/hero}}

<p align="center">
{{#nav}}
  <a href="{{link}}"><b>{{label}}</b></a>{{^last}}&nbsp;&nbsp;·&nbsp;&nbsp;{{/last}}
{{/nav}}
</p>
