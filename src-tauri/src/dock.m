#import <AppKit/AppKit.h>
// Each small Dock process owns one tile and one NSRunningApplication reference.
// It neither patches the browser bundle nor touches unrelated running instances.
@interface Tile:NSObject<NSApplicationDelegate>
@property(strong) NSRunningApplication *browser;
@property BOOL ready;
@end
@implementation Tile
- (void)focus {if(self.ready && !self.browser.terminated){[self.browser activateWithOptions:NSApplicationActivateAllWindows];}}
- (BOOL)applicationShouldHandleReopen:(NSApplication *)app hasVisibleWindows:(BOOL)visible {[self focus];return NO;}
- (void)applicationDidBecomeActive:(NSNotification*)note {[self focus];}
- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication *)sender {if(!self.browser.terminated){[self.browser terminate];return NSTerminateCancel;}return NSTerminateNow;}
@end
int main(int argc,char **argv){@autoreleasepool{
 if(argc!=4)return 2;NSString *num=@(argv[2]);if(num.length<1||num.length>16||[num rangeOfCharacterFromSet:NSCharacterSet.decimalDigitCharacterSet.invertedSet].location!=NSNotFound)return 2;
 [NSApplication sharedApplication];Tile *tile=[Tile new];NSRunningApplication *browser=nil;for(int i=0;i<100;i++){browser=[NSRunningApplication runningApplicationWithProcessIdentifier:atoi(argv[1])];if(browser.executableURL)break;[NSThread sleepForTimeInterval:0.05];}if(!browser||![browser.executableURL.URLByResolvingSymlinksInPath.path isEqualToString:[@(argv[3]) stringByResolvingSymlinksInPath]])return 3;
 tile.browser=browser;NSApp.delegate=tile;[NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];NSImage *base=[[NSWorkspace sharedWorkspace] iconForFile:browser.bundleURL.path];NSImage *image=[NSImage imageWithSize:NSMakeSize(128,128) flipped:NO drawingHandler:^BOOL(NSRect rect){[base drawInRect:rect];NSDictionary *attr=@{NSForegroundColorAttributeName:NSColor.whiteColor,NSFontAttributeName:[NSFont boldSystemFontOfSize:MIN(34,180.0/num.length)]};NSSize size=[num sizeWithAttributes:attr];CGFloat w=MIN(120,MAX(66,size.width+16));NSRect panel=NSMakeRect((128-w)/2,4,w,40);[[NSColor colorWithRed:.86 green:.30 blue:.18 alpha:1]setFill];[[NSBezierPath bezierPathWithRoundedRect:panel xRadius:12 yRadius:12]fill];[num drawAtPoint:NSMakePoint((128-size.width)/2,4+(40-size.height)/2) withAttributes:attr];return YES;}];NSApp.applicationIconImage=image;[NSApp.dockTile display];
 [NSTimer scheduledTimerWithTimeInterval:0.5 repeats:YES block:^(NSTimer *t){tile.ready=YES;if(browser.terminated){[NSApp terminate:nil];}}];[NSApp run];return 0;}}
