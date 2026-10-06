#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

@interface NSPasteboard (Leanterm)
- (NSArray *)getFilePaths;
@end

/// LeantermHostView is the Content view of a Leanterm window.
// It is backed by a Metal CALayer.
@interface LeantermHostView : NSView <CALayerDelegate, NSTextInputClient>
- (LeantermHostView *)initWithFrame:(NSRect)frame
                    metalDevice:(id)metalDevice
             enableTitlebarDrag:(BOOL)enableTitlebarDrag
                       testMode:(BOOL)testMode;
- (void)setAsyncCallback:(BOOL)shouldAsync;
- (void)setPresentsWithTransaction:(BOOL)presentsWithTransaction;
- (BOOL)keyDownImpl:(NSEvent *)event;
@end
