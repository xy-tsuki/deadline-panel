// One GPU background, separable light blur, continuous rounded-rectangle lens.
// Text and controls are drawn afterwards by WebView2, never by this shader.
Texture2D background : register(t0);
SamplerState linearClamp : register(s0);
cbuffer Parameters : register(b0) {
    float4 surface; // window width, height, sampling margin, DPI scale
    float4 origin;  // physical screen x,y; corner radius; dark theme
    float4 layer;   // background image destination rectangle
    float4 clipRect;
    float4 axis;    // blur direction in normalized texture coordinates
};
struct Vertex { float4 position : SV_Position; float2 uv : TEXCOORD0; };
Vertex fullscreen(uint id : SV_VertexID) {
    Vertex v;
    v.uv = float2((id << 1) & 2, id & 2);
    v.position = float4(v.uv * float2(2,-2) + float2(-1,1), 0, 1);
    return v;
}
float4 compose(Vertex v) : SV_Target {
    float2 screenPosition = origin.xy + v.uv * (surface.xy + 2 * surface.z) - surface.z;
    if (any(screenPosition < clipRect.xy) || any(screenPosition >= clipRect.zw)) discard;
    if (axis.z > .5 && axis.z < 1.5) return float4(axis.w > .5 ? float3(.12,.12,.14) : float3(.85,.85,.87),1);
    float2 uv = (screenPosition - layer.xy) / layer.zw;
    if (axis.z > 1.5) uv=frac(uv);
    if (any(uv < 0) || any(uv > 1)) discard;
    return background.Sample(linearClamp, uv);
}
float4 blur(Vertex v) : SV_Target {
    float4 color = background.Sample(linearClamp, v.uv) * 0.227027027;
    color += background.Sample(linearClamp,v.uv+axis.xy*1.384615385) * 0.316216216;
    color += background.Sample(linearClamp,v.uv-axis.xy*1.384615385) * 0.316216216;
    color += background.Sample(linearClamp,v.uv+axis.xy*3.230769231) * 0.070270270;
    color += background.Sample(linearClamp,v.uv-axis.xy*3.230769231) * 0.070270270;
    return color;
}
float4 glass(Vertex v) : SV_Target {
    float2 halfSize = layer.zw * 0.5;
    float radius = min(origin.z, min(halfSize.x,halfSize.y));
    float2 p = v.uv * surface.xy - layer.xy - halfSize;
    float2 q = abs(p) - halfSize + radius;
    float2 outside = max(q,0);
    float distance = length(outside) + min(max(q.x,q.y),0) - radius;
    float alpha = saturate(0.5-distance);
    if (alpha == 0) discard;
    float2 normal = dot(outside,outside)>0.0001 ? normalize(outside) * sign(p)
        : (q.x>q.y ? float2(sign(p.x),0) : float2(0,sign(p.y)));
    float depth = max(0,-distance);
    float edge = 1-smoothstep(0,12*surface.w,depth);
    float2 displacement = normal * (4.0*surface.w*edge*edge) * (axis.z < 0 ? 0 : 1);
    float2 uv = (v.uv*surface.xy+surface.z-displacement)/(surface.xy+2*surface.z);
    float3 color = background.Sample(linearClamp,uv).rgb;
    // Readability remains stable even above a high-contrast wallpaper.
    color = lerp(color, origin.w>0.5 ? float3(.075,.075,.085) : float3(.96,.96,.97), origin.w>0.5 ? .58 : .48);
    if (origin.w>0.5) color *= min(1.0,.40/max(max(color.r,color.g),max(color.b,.001)));
    float rim = exp(-depth/max(.8*surface.w,0.1));
    float illumination = saturate(dot(normal,normalize(float2(-.6,-.8))));
    color += rim*(.055+.10*illumination) + edge*edge*illumination*.035;
    return float4(saturate(color)*alpha,alpha);
}
