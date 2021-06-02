# Masking

## Mask mode

At any time while in the viewport, <kbd>Tab</kbd> may be pressed to enter mask mode. The underlying canvas seen before entering this mode is still shown, but masks are drawn as marching ants (or other optional overlays) above the main document content. While in this mode, an island layer group is provided as the destination for drawing new mask layers using the regular set of tools. The Layer Panel also still shows the underlying main document, which lets the user select layers as contextual inputs for tools that are aware of input layers, like the Fill Tool. Rather than showing the full-color shapes over the main document canvas, they are overlaid in wireframe display mode and surrounded by a marching ants marquee outline. The mask group may be isolated (meaning it becomes the render output to the viewport, and a breadcrumb trail is shown leading from the document to the isolated layer/group) which makes the viewport output show the mask in grayscale and has the Layer Panel host the contents of the mask group. While in mask mode, the working colors are temporarily replaced with a grayscale pair. Certain tools, such as the Freehand Tool and Pen Tool, may default to a "closed" form in mask mode by turning off stroke and setting fill to white in order to provide functionality akin to the lasso or polygonal lasso selection tools. <kbd>Tab</kbd> may be hit again to exit mask mode, but the marching ants still show up. However now, all tools used and commands performed will take into account the working mask. <kbd>Ctrl</kbd><kbd>D</kbd> will discard the working mask.