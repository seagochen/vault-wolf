//
// Created by user on 5/9/25.
//

#ifndef VAULTWOLF_VAULT_SCANNER_SUBSCRIPTION_H
#define VAULTWOLF_VAULT_SCANNER_SUBSCRIPTION_H

#include <string>

struct ScannerSubscription;

class ScannerSubscriptionSamples {
public:
    static ScannerSubscription HotUSStkByVolume();
    static ScannerSubscription TopPercentGainersIbis();
    static ScannerSubscription MostActiveFutEurex();
    static ScannerSubscription HighOptVolumePCRatioUSIndexes();
    static ScannerSubscription ComplexOrdersAndTrades();
};

#endif //VAULTWOLF_VAULT_SCANNER_SUBSCRIPTION_H
