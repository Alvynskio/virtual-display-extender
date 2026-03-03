#ifndef CGVirtualDisplayPrivate_h
#define CGVirtualDisplayPrivate_h

/// Private API declarations for CGVirtualDisplay classes.
/// These classes exist in CoreGraphics at runtime but are not
/// exposed in the public SDK headers.
/// Reverse-engineered from the Objective-C runtime.

#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>

NS_ASSUME_NONNULL_BEGIN

@interface CGVirtualDisplayMode : NSObject
- (instancetype)initWithWidth:(unsigned int)width
                       height:(unsigned int)height
                  refreshRate:(double)refreshRate;
@property (readonly, nonatomic) unsigned int width;
@property (readonly, nonatomic) unsigned int height;
@property (readonly, nonatomic) double refreshRate;
@end

@interface CGVirtualDisplayDescriptor : NSObject
@property (nonatomic, copy) NSString *name;
@property (nonatomic) CGSize sizeInMillimeters;
@property (nonatomic) unsigned int maxPixelsWide;
@property (nonatomic) unsigned int maxPixelsHigh;
@property (nonatomic) unsigned int vendorID;
@property (nonatomic) unsigned int productID;
@property (nonatomic) unsigned int serialNum;
@property (nonatomic, strong) dispatch_queue_t queue;
@property (nonatomic, copy, nullable) void(^terminationHandler)(CGDirectDisplayID displayID);
- (void)setDispatchQueue:(dispatch_queue_t)queue;
@end

@interface CGVirtualDisplaySettings : NSObject
@property (nonatomic, strong) NSArray<CGVirtualDisplayMode *> *modes;
@property (nonatomic) unsigned int hiDPI;
@property (nonatomic) unsigned int rotation;
@end

@interface CGVirtualDisplay : NSObject
- (nullable instancetype)initWithDescriptor:(CGVirtualDisplayDescriptor *)descriptor;
- (BOOL)applySettings:(CGVirtualDisplaySettings *)settings;
@property (readonly, nonatomic) CGDirectDisplayID displayID;
@property (readonly, nonatomic, copy) NSString *name;
@property (readonly, nonatomic) unsigned int vendorID;
@property (readonly, nonatomic) unsigned int productID;
@property (readonly, nonatomic) unsigned int serialNumber;
@end

NS_ASSUME_NONNULL_END

#endif /* CGVirtualDisplayPrivate_h */
